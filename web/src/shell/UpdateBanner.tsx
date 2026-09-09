import { useEffect, useState } from 'react'
import { useInstallUpdate, useUpdate } from '@/api/queries'
import { boot } from '@/boot'
import { Button } from '@/components/ui/button'
import { Spinner } from '@/components/ui/spinner'

/**
 * Shown whenever a newer release exists, on every page, with no way to dismiss it: the only
 * way to make it go away is to update. After installing, the server relaunches itself; we
 * poll until the version changes, then reload.
 */
export function UpdateBanner() {
    const q = useUpdate()
    const install = useInstallUpdate()
    const [restarting, setRestarting] = useState(false)

    useEffect(() => {
        if (!restarting) return
        const t = setInterval(async () => {
            try {
                const r = await fetch('/api/status', { cache: 'no-store' })
                if (r.ok) {
                    const s = (await r.json()) as { version: string }
                    if (s.version !== boot.version) window.location.reload()
                }
            } catch {
                // server is restarting; keep polling
            }
        }, 1500)
        return () => clearInterval(t)
    }, [restarting])

    if (!q.data?.available) return null
    const busy = install.isPending || restarting
    return (
        <div className="border-b border-primary/20 bg-primary/8">
            <div className="mx-auto flex max-w-5xl flex-wrap items-center gap-x-4 gap-y-2 px-6 py-2.5 text-sm">
                <span>
                    <b className="font-medium">Switcheroo v{q.data.latest}</b> is available
                    <span className="text-muted-foreground"> (you have v{q.data.current})</span>.
                </span>
                <span className="ml-auto flex items-center gap-3">
                    {restarting && (
                        <span className="flex items-center gap-2 text-muted-foreground">
                            <Spinner className="size-3.5" /> Installed, restarting…
                        </span>
                    )}
                    <Button
                        size="sm"
                        disabled={busy}
                        onClick={() =>
                            install.mutate(undefined, { onSuccess: () => setRestarting(true) })
                        }
                    >
                        {install.isPending ? 'Downloading…' : 'Update now'}
                    </Button>
                </span>
            </div>
        </div>
    )
}
