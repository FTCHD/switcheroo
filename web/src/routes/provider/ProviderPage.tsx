import { Link, useParams } from 'react-router'
import { useProvider, useSettings } from '@/api/queries'
import { ProviderCard } from '@/routes/dashboard/ProviderCard'

export function ProviderPage() {
    const { id = '' } = useParams()
    const q = useProvider(id)
    const settings = useSettings()
    if (q.isLoading) return <p className="text-muted-foreground">Loading…</p>
    if (q.error || !q.data)
        return <p className="text-destructive">{q.error?.message ?? 'Unknown provider'}</p>
    return (
        <div className="mx-auto max-w-2xl space-y-4">
            <Link to="/" className="text-sm text-muted-foreground hover:underline">
                ← All providers
            </Link>
            {!q.data.installed && (
                <p className="text-sm text-muted-foreground">
                    Not detected on PATH (looked for {q.data.info.binaries.join(', ')}).
                </p>
            )}
            <ProviderCard
                status={q.data}
                confirmSwitch={settings.data?.tray.confirm_switch ?? false}
                expanded
            />
        </div>
    )
}
