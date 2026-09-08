import { CheckIcon, XIcon } from 'lucide-react'
import { Link } from 'react-router'
import { useDoctor } from '@/api/queries'
import { Skeleton } from '@/components/ui/skeleton'
import { vaultName } from '@/lib/labels'
import { cn } from '@/lib/ui'
import { ProviderIcon } from '@/routes/dashboard/ProviderIcon'
import { TierBadge } from '@/routes/dashboard/TierBadge'
import { WarningList } from '@/routes/dashboard/WarningList'
import { PageHeader } from '@/shell/PageHeader'
import { Panel } from '@/shell/Panel'

function Fact({ label, children }: { label: string; children: React.ReactNode }) {
    return (
        <div className="grid gap-1 px-6 py-3.5 sm:grid-cols-[11rem_1fr] sm:gap-6">
            <dt className="text-sm text-muted-foreground">{label}</dt>
            <dd className="min-w-0 font-mono text-[13px] break-all">{children}</dd>
        </div>
    )
}

export function DoctorPage() {
    const q = useDoctor()
    return (
        <>
            <PageHeader
                title="Doctor"
                description="Everything Switcheroo can see: where secrets go, which CLIs are on PATH, who they are signed in as, and anything that would make a switch silently do nothing."
            />
            {q.isLoading && (
                <Panel>
                    {[0, 1, 2, 3].map((i) => (
                        <div key={i} className="px-6 py-4">
                            <Skeleton className="h-4 w-1/2" />
                        </div>
                    ))}
                </Panel>
            )}
            {q.error && <p className="text-destructive">{q.error.message}</p>}
            {q.data && (
                <>
                    <Panel eyebrow="This machine">
                        <dl className="divide-y divide-border/70">
                            <Fact label="Switcheroo">
                                v{q.data.version} · {q.data.os}
                            </Fact>
                            <Fact label="Secrets">
                                <span className="inline-flex items-center gap-1.5">
                                    {q.data.vault_ok ? (
                                        <CheckIcon
                                            className="size-3.5 text-primary"
                                            strokeWidth={2.5}
                                        />
                                    ) : (
                                        <XIcon
                                            className="size-3.5 text-destructive"
                                            strokeWidth={2.5}
                                        />
                                    )}
                                    {vaultName(q.data.vault)}
                                </span>
                                {q.data.vault_error && (
                                    <p className="mt-1 font-sans text-[13px] text-destructive text-pretty">
                                        {q.data.vault_error}
                                    </p>
                                )}
                            </Fact>
                            <Fact label="State">{q.data.data_dir}</Fact>
                            <Fact label="PATH">
                                <details className="group">
                                    <summary className="cursor-pointer list-none text-muted-foreground select-none hover:text-foreground">
                                        {q.data.path.length} directories searched
                                        <span className="ml-1 text-[11px] group-open:hidden">
                                            · show
                                        </span>
                                        <span className="ml-1 hidden text-[11px] group-open:inline">
                                            · hide
                                        </span>
                                    </summary>
                                    <div className="mt-3 max-w-xl space-y-2 font-sans text-[13px] leading-relaxed break-normal text-foreground/80 text-pretty">
                                        <p>
                                            Switcheroo only finds CLIs that live in one of these
                                            directories. A tool installed inside a project, such as{' '}
                                            <code className="font-mono">wrangler</code> in a repo's{' '}
                                            <code className="font-mono">node_modules/.bin</code>, is
                                            invisible here even though its login file is global and
                                            would switch fine.
                                        </p>
                                        <p>
                                            To manage such a tool, install it globally (for example{' '}
                                            <code className="font-mono">npm i -g wrangler</code>) or
                                            add its directory to PATH. If a CLI works in your
                                            terminal but is missing below, your shell adds a
                                            directory that Switcheroo did not pick up; start it from
                                            that terminal or add the directory to your login shell's
                                            PATH.
                                        </p>
                                    </div>
                                    <ul className="mt-3 space-y-0.5 text-muted-foreground">
                                        {q.data.path.map((p) => (
                                            <li key={p}>{p}</li>
                                        ))}
                                    </ul>
                                </details>
                            </Fact>
                        </dl>
                    </Panel>

                    <Panel eyebrow="CLIs">
                        {q.data.providers.map((p) => (
                            <div
                                key={p.info.id}
                                className={cn(
                                    'grid gap-x-6 gap-y-2 px-6 py-4 sm:grid-cols-[1fr_1fr_1fr]',
                                    !p.installed && 'opacity-60'
                                )}
                            >
                                <div className="flex items-center gap-3">
                                    <ProviderIcon id={p.info.id} size="sm" muted={!p.installed} />
                                    <div className="min-w-0">
                                        <div className="flex items-center gap-2">
                                            <Link
                                                to={`/providers/${p.info.id}`}
                                                className="text-sm font-medium hover:underline"
                                            >
                                                {p.info.name}
                                            </Link>
                                            <TierBadge tier={p.info.tier} />
                                        </div>
                                    </div>
                                </div>
                                <div className="min-w-0 font-mono text-[12px] break-all">
                                    {p.installed ? (
                                        <>
                                            <div>{p.installed.path}</div>
                                            <div className="text-muted-foreground tabular-nums">
                                                {p.installed.version}
                                            </div>
                                        </>
                                    ) : (
                                        <span className="text-muted-foreground">
                                            not found · {p.info.binaries.join(', ')}
                                        </span>
                                    )}
                                </div>
                                <div className="min-w-0">
                                    <div className="text-sm">
                                        {p.live ? (
                                            p.live.label
                                        ) : p.installed ? (
                                            <span className="text-muted-foreground">
                                                not signed in
                                            </span>
                                        ) : (
                                            <span className="text-muted-foreground/50">—</span>
                                        )}
                                    </div>
                                    <WarningList warnings={p.warnings} className="mt-1.5" />
                                </div>
                            </div>
                        ))}
                    </Panel>
                </>
            )}
        </>
    )
}
