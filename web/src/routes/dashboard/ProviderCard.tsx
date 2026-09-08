import { RefreshCwIcon } from 'lucide-react'
import { Link } from 'react-router'
import { useLogin, useRefresh, useSave } from '@/api/queries'
import type { ProviderStatus } from '@/api/types'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { AccountRow } from './AccountRow'
import { TierBadge } from './TierBadge'
import { WarningList } from './WarningList'

export function ProviderCard({
    status,
    confirmSwitch,
    expanded = false,
}: {
    status: ProviderStatus
    confirmSwitch: boolean
    expanded?: boolean
}) {
    const save = useSave()
    const login = useLogin()
    const refresh = useRefresh()
    const { info, live, accounts, active_account } = status
    const liveUnsaved = live !== null && active_account === null
    const detail = live?.extra?.org ?? live?.extra?.plan ?? live?.extra?.name

    return (
        <Card>
            <CardHeader className="pb-3">
                <div className="flex items-center gap-2">
                    <CardTitle>
                        {expanded ? (
                            info.name
                        ) : (
                            <Link to={`/providers/${info.id}`}>{info.name}</Link>
                        )}
                    </CardTitle>
                    <TierBadge tier={info.tier} />
                    <span className="ml-auto text-xs text-muted-foreground">
                        {status.installed?.version ?? ''}
                    </span>
                    <Button
                        size="icon-sm"
                        variant="ghost"
                        aria-label="Refresh"
                        title="Ask the CLI who is logged in"
                        onClick={() => refresh.mutate(info.id)}
                        disabled={refresh.isPending}
                    >
                        <RefreshCwIcon className={refresh.isPending ? 'animate-spin' : ''} />
                    </Button>
                </div>
                <div className="text-sm">
                    {live ? (
                        <>
                            <span className="text-muted-foreground">Logged in as </span>
                            <span className="font-medium">{live.label}</span>
                            {detail && <span className="text-muted-foreground"> · {detail}</span>}
                        </>
                    ) : (
                        <span className="text-muted-foreground">Not logged in</span>
                    )}
                </div>
            </CardHeader>
            <CardContent className="space-y-3">
                <WarningList warnings={status.warnings} />
                {accounts.length > 0 ? (
                    <div className="space-y-2">
                        {accounts.map((a) => (
                            <AccountRow
                                key={a.id}
                                account={a}
                                active={a.id === active_account}
                                confirmSwitch={confirmSwitch}
                            />
                        ))}
                    </div>
                ) : (
                    <p className="text-sm text-muted-foreground">No saved accounts yet.</p>
                )}
                <div className="flex flex-wrap gap-2">
                    {liveUnsaved && (
                        <Button
                            size="sm"
                            variant="outline"
                            onClick={() => save.mutate({ provider: info.id })}
                            disabled={save.isPending}
                        >
                            Save current login
                        </Button>
                    )}
                    <Button
                        size="sm"
                        variant="outline"
                        onClick={() => login.mutate(info.id)}
                        disabled={login.isPending}
                        title={`Opens a terminal running: ${info.login_command.join(' ')}`}
                    >
                        Add account…
                    </Button>
                </div>
                {expanded && (
                    <div className="space-y-2 border-t pt-3 text-xs text-muted-foreground">
                        <p>{info.notes}</p>
                        {status.slots.length > 0 && (
                            <div>
                                <div className="font-medium text-foreground">Touches</div>
                                <ul className="list-disc pl-4">
                                    {status.slots.map((s) => (
                                        <li key={s}>{s}</li>
                                    ))}
                                </ul>
                            </div>
                        )}
                        {info.env_shadow.length > 0 && (
                            <p>
                                Ignored when set: <code>{info.env_shadow.join(', ')}</code>
                            </p>
                        )}
                        {info.restart_hint && <p>{info.restart_hint}</p>}
                        {status.installed && <p>Binary: {status.installed.path}</p>}
                    </div>
                )}
            </CardContent>
        </Card>
    )
}
