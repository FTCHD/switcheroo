import { Link } from 'react-router'
import { useProviders, useSettings } from '@/api/queries'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { PageHeader } from '@/shell/PageHeader'
import { Panel } from '@/shell/Panel'
import { ProviderIcon } from './ProviderIcon'
import { ProviderRow } from './ProviderRow'

function RosterSkeleton() {
    return (
        <Panel>
            {[0, 1, 2].map((i) => (
                <div key={i} className="flex items-start gap-5 p-6 sm:px-7">
                    <Skeleton className="size-11 rounded-2xl" />
                    <div className="flex-1 space-y-3">
                        <Skeleton className="h-3 w-24" />
                        <Skeleton className="h-7 w-72 max-w-full" />
                        <Skeleton className="h-9 w-40 rounded-full" />
                    </div>
                </div>
            ))}
        </Panel>
    )
}

export function Dashboard() {
    const providers = useProviders()
    const settings = useSettings()
    const hidden = new Set(settings.data?.hidden_providers ?? [])
    const all = (providers.data ?? []).filter((p) => !hidden.has(p.info.id))
    const installed = all.filter((p) => p.installed)
    const missing = all.filter((p) => !p.installed)
    const signedIn = installed.filter((p) => p.live).length

    return (
        <>
            <PageHeader
                title="Accounts"
                description={
                    providers.data
                        ? `${installed.length} ${installed.length === 1 ? 'CLI' : 'CLIs'} on this machine, ${signedIn} signed in. Pick a remembered account to switch; the current login is remembered first.`
                        : 'Who each CLI on this machine is signed in as, and the accounts you can switch to.'
                }
            />

            {providers.isLoading && <RosterSkeleton />}

            {providers.error && (
                <div className="panel p-8">
                    <p className="font-heading text-lg font-medium">The server did not answer</p>
                    <p className="mt-1 text-sm text-muted-foreground">{providers.error.message}</p>
                </div>
            )}

            {providers.data && installed.length === 0 && (
                <div className="rounded-[calc(var(--radius)+14px)] border border-dashed border-border px-8 py-14 text-center">
                    <p className="font-heading text-xl font-medium">Nothing to switch yet</p>
                    <p className="mx-auto mt-2 max-w-md text-sm text-muted-foreground text-pretty">
                        None of the supported CLIs were found on PATH. Install one and sign in, then
                        come back. Doctor lists every tool Switcheroo looks for and where.
                    </p>
                    <Button className="mt-6" variant="outline" render={<Link to="/doctor" />}>
                        Open Doctor
                    </Button>
                </div>
            )}

            {installed.length > 0 && (
                <Panel>
                    {installed.map((p, i) => (
                        <ProviderRow
                            key={p.info.id}
                            status={p}
                            index={i}
                            confirmSwitch={settings.data?.tray.confirm_switch ?? false}
                        />
                    ))}
                </Panel>
            )}

            {missing.length > 0 && (
                <section>
                    <h2 className="mb-3 px-1 font-heading text-[12px] font-semibold tracking-[0.1em] text-muted-foreground uppercase">
                        Not on this machine
                    </h2>
                    <ul className="flex flex-wrap gap-2">
                        {missing.map((p) => (
                            <li key={p.info.id}>
                                <Link
                                    to={`/providers/${p.info.id}`}
                                    className="flex items-center gap-2 rounded-full border border-dashed border-border py-1 pr-3.5 pl-1 text-sm text-muted-foreground transition-colors duration-150 hover:border-foreground/30 hover:text-foreground"
                                >
                                    <ProviderIcon id={p.info.id} muted size="sm" />
                                    {p.info.name}
                                </Link>
                            </li>
                        ))}
                    </ul>
                </section>
            )}
        </>
    )
}
