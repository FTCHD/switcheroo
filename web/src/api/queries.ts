// react-query hooks over the API. SSE (events.ts) invalidates these on server-side changes.

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { toast } from '@/components/ui/toast'
import { api } from './client'
import type {
    Account,
    Autostart,
    Doctor,
    ProviderStatus,
    ServerStatus,
    Settings,
    SwitchOutcome,
    Usage,
} from './types'

export const keys = {
    providers: ['providers'] as const,
    provider: (id: string) => ['providers', id] as const,
    settings: ['settings'] as const,
    doctor: ['doctor'] as const,
    status: ['status'] as const,
    autostart: ['autostart'] as const,
    usage: (id: string) => ['usage', id] as const,
}

const notify = {
    success: (title: string, description?: string) =>
        toast.add({ title, description, type: 'success' }),
    info: (title: string, description?: string) => toast.add({ title, description, type: 'info' }),
    warning: (title: string, description?: string, timeout?: number) =>
        toast.add({ title, description, type: 'warning', timeout }),
    error: (title: string, description?: string) =>
        toast.add({ title, description, type: 'error', timeout: 8000 }),
}

export function useServerStatus() {
    return useQuery({ queryKey: keys.status, queryFn: () => api.get<ServerStatus>('/api/status') })
}

export function useProviders() {
    return useQuery({
        queryKey: keys.providers,
        queryFn: () => api.get<ProviderStatus[]>('/api/providers'),
    })
}

export function useProvider(id: string) {
    return useQuery({
        queryKey: keys.provider(id),
        queryFn: () => api.get<ProviderStatus>(`/api/providers/${encodeURIComponent(id)}`),
    })
}

export function useSettings() {
    return useQuery({ queryKey: keys.settings, queryFn: () => api.get<Settings>('/api/settings') })
}

export function useDoctor() {
    return useQuery({ queryKey: keys.doctor, queryFn: () => api.get<Doctor>('/api/doctor') })
}

function useInvalidate() {
    const qc = useQueryClient()
    return () => {
        qc.invalidateQueries({ queryKey: keys.providers })
        qc.invalidateQueries({ queryKey: keys.doctor })
    }
}

export function useSwitch() {
    const invalidate = useInvalidate()
    return useMutation({
        mutationFn: ({ provider, account }: { provider: string; account: string }) =>
            api.post<SwitchOutcome>(`/api/providers/${encodeURIComponent(provider)}/use`, {
                account,
            }),
        onSuccess: (out) => {
            notify.success(
                `Switched to ${out.account.label}`,
                out.warnings.map((w) => w.message).join('\n') || undefined
            )
            invalidate()
        },
        onError: (e: Error) => notify.error('Switch failed', e.message),
    })
}

export function useSave() {
    const invalidate = useInvalidate()
    return useMutation({
        mutationFn: ({ provider, label }: { provider: string; label?: string }) =>
            api.post<Account>(`/api/providers/${encodeURIComponent(provider)}/save`, { label }),
        onSuccess: (a) => {
            notify.success(`Remembered ${a.label}`)
            invalidate()
        },
        onError: (e: Error) => notify.error('Could not remember the login', e.message),
    })
}

export function useLogin() {
    return useMutation({
        mutationFn: (provider: string) =>
            api.post<{ spawned: boolean; command: string }>(
                `/api/providers/${encodeURIComponent(provider)}/login`
            ),
        onSuccess: (r) => {
            if (r.spawned) {
                notify.info(
                    'Sign in in the terminal that just opened',
                    'When the CLI finishes, the new account appears here.'
                )
            } else {
                notify.warning('Run this in a terminal', r.command, 15000)
            }
        },
        onError: (e: Error) => notify.error('Could not open a terminal', e.message),
    })
}

export function useRefresh() {
    const invalidate = useInvalidate()
    return useMutation({
        mutationFn: (provider: string) =>
            api.post<ProviderStatus>(`/api/providers/${encodeURIComponent(provider)}/refresh`),
        onSuccess: invalidate,
        onError: (e: Error) => notify.error('Could not ask the CLI', e.message),
    })
}

export function useRemove() {
    const invalidate = useInvalidate()
    return useMutation({
        mutationFn: ({ provider, account }: { provider: string; account: string }) =>
            api.del<Account>(
                `/api/accounts/${encodeURIComponent(provider)}/${encodeURIComponent(account)}`
            ),
        onSuccess: (a) => {
            notify.success(`Forgot ${a.label}`)
            invalidate()
        },
        onError: (e: Error) => notify.error('Could not forget the account', e.message),
    })
}

export function useRename() {
    const invalidate = useInvalidate()
    return useMutation({
        mutationFn: ({
            provider,
            account,
            label,
        }: {
            provider: string
            account: string
            label: string
        }) =>
            api.patch<Account>(
                `/api/accounts/${encodeURIComponent(provider)}/${encodeURIComponent(account)}`,
                { label }
            ),
        onSuccess: invalidate,
        onError: (e: Error) => notify.error('Could not rename the account', e.message),
    })
}

export function useUsage(id: string, enabled: boolean) {
    return useQuery({
        queryKey: keys.usage(id),
        queryFn: () => api.get<Usage | null>(`/api/providers/${encodeURIComponent(id)}/usage`),
        enabled,
        staleTime: 60_000,
        refetchInterval: 120_000,
        retry: false,
    })
}

export function useRefreshUsage(id: string) {
    const qc = useQueryClient()
    return useMutation({
        mutationFn: () =>
            api.get<Usage | null>(`/api/providers/${encodeURIComponent(id)}/usage?refresh=true`),
        onSuccess: (u) => qc.setQueryData(keys.usage(id), u),
        onError: (e: Error) => notify.error('Could not refresh usage', e.message),
    })
}

export function useAutostart() {
    return useQuery({
        queryKey: keys.autostart,
        queryFn: () => api.get<Autostart>('/api/autostart'),
    })
}

export function useSetAutostart() {
    const qc = useQueryClient()
    return useMutation({
        mutationFn: (enabled: boolean) => api.put<Autostart>('/api/autostart', { enabled }),
        onSuccess: (a) => {
            qc.setQueryData(keys.autostart, a)
            notify.success(
                a.enabled ? 'Switcheroo starts at login' : 'Switcheroo no longer starts at login',
                a.enabled ? 'The tray opens the next time you sign in to this machine.' : undefined
            )
        },
        onError: (e: Error) => notify.error('Could not change start at login', e.message),
    })
}

export function useSaveSettings() {
    const qc = useQueryClient()
    return useMutation({
        mutationFn: (s: Settings) => api.put<Settings>('/api/settings', s),
        onSuccess: (s) => {
            qc.setQueryData(keys.settings, s)
            notify.success('Settings saved')
        },
        onError: (e: Error) => notify.error('Could not save settings', e.message),
    })
}
