import { Link } from 'react-router'
import { useProviders, useSettings } from '@/api/queries'
import { Badge } from '@/components/ui/badge'
import { ProviderCard } from './ProviderCard'

export function Dashboard() {
    const providers = useProviders()
    const settings = useSettings()
    if (providers.isLoading) return <p className="text-muted-foreground">Detecting CLIs…</p>
    if (providers.error)
        return (
            <p className="text-destructive">Could not load providers: {providers.error.message}</p>
        )
    const hidden = new Set(settings.data?.hidden_providers ?? [])
    const all = (providers.data ?? []).filter((p) => !hidden.has(p.info.id))
    const installed = all.filter((p) => p.installed)
    const missing = all.filter((p) => !p.installed)
    return (
        <div className="space-y-6">
            {installed.length === 0 && (
                <p className="text-muted-foreground">
                    No supported CLI was found on PATH. See Doctor for what was searched.
                </p>
            )}
            <div className="grid gap-4 md:grid-cols-2">
                {installed.map((p) => (
                    <ProviderCard
                        key={p.info.id}
                        status={p}
                        confirmSwitch={settings.data?.tray.confirm_switch ?? false}
                    />
                ))}
            </div>
            {missing.length > 0 && (
                <div className="text-sm text-muted-foreground">
                    <span className="mr-2">Not detected:</span>
                    {missing.map((p) => (
                        <Link key={p.info.id} to={`/providers/${p.info.id}`} className="mr-2">
                            <Badge variant="outline">{p.info.name}</Badge>
                        </Link>
                    ))}
                </div>
            )}
        </div>
    )
}
