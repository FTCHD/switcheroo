// react-query hooks over the API. SSE (events.ts) invalidates these on server-side changes.

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'
import { api } from './client'
import type {
    Account,
    Doctor,
    ProviderStatus,
    ServerStatus,
    Settings,
    SwitchOutcome,
} from './types'

export const keys = {
    providers: ['providers'] as const,
    provider: (id: string) => ['providers', id] as const,
    settings: ['settings'] as const,
    doctor: ['doctor'] as const,
    status: ['status'] as const,
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
            toast.success(`Switched to ${out.account.label}`, {
                description: out.warnings.map((w) => w.message).join('\n') || undefined,
            })
            invalidate()
        },
        onError: (e: Error) => toast.error('Switch failed', { description: e.message }),
    })
}

export function useSave() {
    const invalidate = useInvalidate()
    return useMutation({
        mutationFn: ({ provider, label }: { provider: string; label?: string }) =>
            api.post<Account>(`/api/providers/${encodeURIComponent(provider)}/save`, { label }),
        onSuccess: (a) => {
            toast.success(`Saved ${a.label}`)
            invalidate()
        },
        onError: (e: Error) => toast.error('Could not save the login', { description: e.message }),
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
                toast.info('A terminal window was opened for the login', {
                    description: 'Finish the login there; this page updates automatically.',
                })
            } else {
                toast.warning('Run this in a terminal', { description: r.command, duration: 15000 })
            }
        },
        onError: (e: Error) => toast.error('Could not start the login', { description: e.message }),
    })
}

export function useRefresh() {
    const invalidate = useInvalidate()
    return useMutation({
        mutationFn: (provider: string) =>
            api.post<ProviderStatus>(`/api/providers/${encodeURIComponent(provider)}/refresh`),
        onSuccess: invalidate,
        onError: (e: Error) => toast.error('Refresh failed', { description: e.message }),
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
            toast.success(`Removed ${a.label}`)
            invalidate()
        },
        onError: (e: Error) => toast.error('Remove failed', { description: e.message }),
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
        onError: (e: Error) => toast.error('Rename failed', { description: e.message }),
    })
}

export function useSaveSettings() {
    const qc = useQueryClient()
    return useMutation({
        mutationFn: (s: Settings) => api.put<Settings>('/api/settings', s),
        onSuccess: (s) => {
            qc.setQueryData(keys.settings, s)
            toast.success('Settings saved')
        },
        onError: (e: Error) => toast.error('Could not save settings', { description: e.message }),
    })
}
