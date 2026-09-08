import { ChevronRightIcon, PlusIcon, RefreshCwIcon } from 'lucide-react'
import { Link } from 'react-router'
import { useLogin, useRefresh, useSave } from '@/api/queries'
import type { ProviderStatus } from '@/api/types'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/ui'
import { AccountChip } from './AccountChip'
import { ProviderIcon } from './ProviderIcon'
import { TierBadge } from './TierBadge'
import { UsageStrip } from './UsageStrip'
import { WarningList } from './WarningList'

function LiveDot({
    off = false,
    color,
    className,
}: {
    off?: boolean
    color?: string
    className?: string
}) {
    return (
        <span aria-hidden="true" className={cn('relative flex size-2.5 shrink-0', className)}>
            {!off && (
                <span
                    style={color ? { borderColor: color } : undefined}
                    className="absolute inset-0 rounded-full border-[1.5px] border-primary motion-safe:animate-live"
                />
            )}
            <span
                style={off || !color ? undefined : { backgroundColor: color }}
                className={cn(
                    'relative size-2.5 rounded-full',
                    off ? 'border border-border bg-transparent' : 'bg-primary'
                )}
            />
        </span>
    )
}

/** A dashed "empty jack" on the switchboard: an action that produces a chip. */
function JackButton({
    children,
    primary = false,
    ...props
}: React.ComponentProps<'button'> & { primary?: boolean }) {
    return (
        <button
            type="button"
            {...props}
            className={cn(
                'flex h-9 items-center gap-1.5 rounded-full border border-dashed pr-3.5 pl-2.5 text-sm font-medium transition-[color,background-color,border-color,scale] duration-150 outline-none select-none focus-visible:ring-3 focus-visible:ring-ring/30 active:scale-[0.96] disabled:pointer-events-none disabled:opacity-50',
                primary
                    ? 'border-primary/50 text-link hover:border-primary hover:bg-primary/8'
                    : 'border-border text-muted-foreground hover:border-foreground/30 hover:text-foreground'
            )}
        >
            <PlusIcon className="size-3.5" strokeWidth={2} />
            {children}
        </button>
    )
}

function Eyebrow({ status, linkTitle }: { status: ProviderStatus; linkTitle: boolean }) {
    const { info } = status
    const cls =
        'font-heading text-[12px] font-semibold tracking-[0.1em] text-muted-foreground uppercase'
    return (
        <div className="flex items-center gap-2">
            {linkTitle ? (
                <Link
                    to={`/providers/${info.id}`}
                    title={`${info.name}: details and how switching works`}
                    className={cn(
                        cls,
                        'group/title flex items-center gap-0.5 underline-offset-4 transition-colors duration-150 hover:text-foreground hover:underline hover:decoration-dotted'
                    )}
                >
                    {info.name}
                    <ChevronRightIcon
                        className="size-3 opacity-60 transition-[translate,opacity] duration-150 group-hover/title:translate-x-0.5 group-hover/title:opacity-100"
                        strokeWidth={2.5}
                    />
                </Link>
            ) : (
                <span className={cls}>{info.name}</span>
            )}
            <TierBadge tier={info.tier} />
        </div>
    )
}

function RefreshButton({ status }: { status: ProviderStatus }) {
    const refresh = useRefresh()
    return (
        <Button
            size="icon-xs"
            variant="ghost"
            className="text-muted-foreground/70 hover:text-foreground"
            aria-label={`Ask ${status.info.name} who is signed in`}
            title="Ask the CLI who is signed in"
            onClick={() => refresh.mutate(status.info.id)}
            disabled={refresh.isPending}
        >
            <RefreshCwIcon strokeWidth={1.75} className={refresh.isPending ? 'animate-spin' : ''} />
        </Button>
    )
}

/**
 * One CLI on the roster. Signed-in tools get the full treatment (identity set large, then
 * the switchboard of remembered accounts); idle tools with nothing remembered collapse to a
 * single quiet line so the identities stay the hero.
 */
export function ProviderRow({
    status,
    confirmSwitch,
    index = 0,
    linkTitle = true,
}: {
    status: ProviderStatus
    confirmSwitch: boolean
    index?: number
    linkTitle?: boolean
}) {
    const save = useSave()
    const login = useLogin()
    const { info, live, accounts, active_account } = status
    const liveUnsaved = live !== null && active_account === null
    const compact = live === null && accounts.length === 0
    const detail = [live?.extra?.org ?? live?.extra?.name, live?.extra?.plan]
        .filter(Boolean)
        .join(' · ')
    const delay = { animationDelay: `${Math.min(index, 8) * 60}ms` }

    if (compact) {
        return (
            <article
                className="flex items-center gap-4 px-6 py-3.5 motion-safe:animate-rise sm:px-7"
                style={delay}
            >
                <ProviderIcon id={info.id} color={info.color} size="sm" muted />
                <div className="flex min-w-0 flex-1 flex-wrap items-center gap-x-4 gap-y-1">
                    <Eyebrow status={status} linkTitle={linkTitle} />
                    <WarningList warnings={status.warnings} className="basis-full" />
                </div>
                <JackButton onClick={() => login.mutate(info.id)} disabled={login.isPending}>
                    Sign in
                </JackButton>
                <RefreshButton status={status} />
            </article>
        )
    }

    return (
        <article className="p-6 motion-safe:animate-rise sm:px-7" style={delay}>
            <div className="flex items-start gap-5">
                <ProviderIcon id={info.id} color={info.color} />
                <div className="min-w-0 flex-1">
                    <div className="flex items-center">
                        <Eyebrow status={status} linkTitle={linkTitle} />
                        <div className="ml-auto">
                            <RefreshButton status={status} />
                        </div>
                    </div>

                    <div className="mt-2 flex items-start gap-3">
                        <LiveDot off={!live} color={info.color} className="mt-[11px]" />
                        <div className="min-w-0">
                            {live ? (
                                <>
                                    <p className="font-heading text-[24px] leading-[1.15] font-medium tracking-[-0.015em] break-words sm:text-[26px]">
                                        {live.label}
                                    </p>
                                    {detail && (
                                        <p className="mt-1 text-sm text-muted-foreground text-pretty">
                                            {detail}
                                        </p>
                                    )}
                                </>
                            ) : (
                                <p className="font-heading text-[24px] leading-[1.15] font-medium tracking-[-0.015em] text-muted-foreground/50 sm:text-[26px]">
                                    Not signed in
                                </p>
                            )}
                        </div>
                    </div>

                    <UsageStrip
                        providerId={info.id}
                        color={info.color}
                        enabled={live !== null && info.supports_usage}
                        expanded={!linkTitle}
                    />

                    <div className="mt-5 flex flex-wrap items-center gap-2">
                        {accounts.map((a) => (
                            <AccountChip
                                key={a.id}
                                account={a}
                                active={a.id === active_account}
                                confirmSwitch={confirmSwitch}
                            />
                        ))}
                        {liveUnsaved && (
                            <JackButton
                                primary
                                onClick={() => save.mutate({ provider: info.id })}
                                disabled={save.isPending}
                                title="Keep this login so you can switch back to it later"
                            >
                                Remember this login
                            </JackButton>
                        )}
                        <JackButton
                            onClick={() => login.mutate(info.id)}
                            disabled={login.isPending}
                            title={`Opens a terminal running: ${info.login_command.join(' ')}`}
                        >
                            {live ? 'Sign in to another account' : 'Sign in'}
                        </JackButton>
                    </div>

                    <WarningList warnings={status.warnings} className="mt-4" />
                </div>
            </div>
        </article>
    )
}
