import { ArrowLeftIcon } from 'lucide-react'
import { Link, useParams } from 'react-router'
import { useProvider, useSettings } from '@/api/queries'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { strategyName } from '@/lib/labels'
import { ProviderRow } from '@/routes/dashboard/ProviderRow'
import { PageHeader } from '@/shell/PageHeader'
import { Panel } from '@/shell/Panel'

function Detail({ label, children }: { label: string; children: React.ReactNode }) {
    return (
        <div className="grid gap-1 px-6 py-4 sm:grid-cols-[11rem_1fr] sm:gap-6">
            <dt className="text-sm text-muted-foreground">{label}</dt>
            <dd className="min-w-0 text-sm">{children}</dd>
        </div>
    )
}

export function ProviderPage() {
    const { id = '' } = useParams()
    const q = useProvider(id)
    const settings = useSettings()

    return (
        <>
            <Button
                variant="link"
                size="sm"
                className="-ml-2 mb-4 text-muted-foreground"
                render={<Link to="/" />}
            >
                <ArrowLeftIcon data-icon="inline-start" strokeWidth={1.75} />
                All accounts
            </Button>

            {q.isLoading && (
                <div className="space-y-4">
                    <Skeleton className="h-9 w-64" />
                    <Skeleton className="h-4 w-96 max-w-full" />
                </div>
            )}
            {q.error && <p className="text-destructive">{q.error.message}</p>}

            {q.data && (
                <>
                    <PageHeader
                        eyebrow={strategyName(q.data.info.strategy)}
                        title={q.data.info.name}
                        description={q.data.info.notes}
                    />
                    {q.data.installed ? (
                        <Panel>
                            <ProviderRow
                                status={q.data}
                                confirmSwitch={settings.data?.tray.confirm_switch ?? false}
                                linkTitle={false}
                            />
                        </Panel>
                    ) : (
                        <div className="panel mb-8 px-7 py-6">
                            <p className="font-heading text-lg font-medium">Not on this machine</p>
                            <p className="mt-1 max-w-xl text-sm text-muted-foreground text-pretty">
                                Switcheroo looked for{' '}
                                <code className="font-mono">{q.data.info.binaries.join(', ')}</code>{' '}
                                on PATH and found nothing. Install the CLI and sign in, then this
                                page fills in.
                            </p>
                            <p className="mt-3 max-w-xl text-sm text-muted-foreground text-pretty">
                                Installed only inside a project, for example in a repo's{' '}
                                <code className="font-mono">node_modules/.bin</code>? That copy is
                                not visible from here even though its login is global. Install it
                                globally or add its directory to PATH; Doctor lists the directories
                                searched.
                            </p>
                        </div>
                    )}

                    <Panel eyebrow="How it works">
                        <dl className="divide-y divide-border/70">
                            <Detail label="Touches">
                                <ul className="space-y-1 font-mono text-[13px] break-all">
                                    {q.data.slots.map((s) => (
                                        <li key={s}>{s}</li>
                                    ))}
                                </ul>
                            </Detail>
                            <Detail label="Sign-in command">
                                <code className="font-mono text-[13px]">
                                    {q.data.info.login_command.join(' ')}
                                </code>
                            </Detail>
                            {q.data.info.env_shadow.length > 0 && (
                                <Detail label="Ignored when set">
                                    <div className="flex flex-wrap gap-1.5">
                                        {q.data.info.env_shadow.map((v) => (
                                            <code
                                                key={v}
                                                className="rounded-md bg-muted px-1.5 py-0.5 font-mono text-[12px]"
                                            >
                                                {v}
                                            </code>
                                        ))}
                                    </div>
                                    <p className="mt-1.5 text-[13px] text-muted-foreground text-pretty">
                                        The CLI prefers these environment variables over its stored
                                        login, so a switch has no effect while one is set.
                                    </p>
                                </Detail>
                            )}
                            {q.data.info.restart_hint && (
                                <Detail label="After switching">{q.data.info.restart_hint}</Detail>
                            )}
                            {q.data.installed && (
                                <Detail label="Binary">
                                    <span className="font-mono text-[13px] break-all">
                                        {q.data.installed.path}
                                    </span>
                                    {q.data.installed.version && (
                                        <span className="ml-2 font-mono text-[12px] text-muted-foreground tabular-nums">
                                            {q.data.installed.version}
                                        </span>
                                    )}
                                </Detail>
                            )}
                        </dl>
                    </Panel>
                </>
            )}
        </>
    )
}
