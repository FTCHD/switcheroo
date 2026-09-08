import { useDoctor } from '@/api/queries'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'

export function DoctorPage() {
    const q = useDoctor()
    if (q.isLoading) return <p className="text-muted-foreground">Running checks…</p>
    if (q.error || !q.data) return <p className="text-destructive">{q.error?.message}</p>
    const d = q.data
    return (
        <div className="space-y-4">
            <Card>
                <CardHeader>
                    <CardTitle>Environment</CardTitle>
                </CardHeader>
                <CardContent className="grid gap-1 text-sm sm:grid-cols-2">
                    <div>Version</div>
                    <div>{d.version}</div>
                    <div>OS</div>
                    <div>{d.os}</div>
                    <div>Data dir</div>
                    <div className="break-all">{d.data_dir}</div>
                    <div>Vault</div>
                    <div>
                        {d.vault} {d.vault_ok ? '✓' : `✗ ${d.vault_error}`}
                    </div>
                    <div>PATH</div>
                    <div className="break-all text-xs text-muted-foreground">
                        {d.path.join(' : ')}
                    </div>
                </CardContent>
            </Card>
            <Card>
                <CardHeader>
                    <CardTitle>Providers</CardTitle>
                </CardHeader>
                <CardContent>
                    <table className="w-full text-sm">
                        <thead className="text-left text-muted-foreground">
                            <tr>
                                <th className="py-1 pr-3">Provider</th>
                                <th className="py-1 pr-3">Binary</th>
                                <th className="py-1 pr-3">Logged in</th>
                                <th className="py-1 pr-3">Touches</th>
                                <th className="py-1">Findings</th>
                            </tr>
                        </thead>
                        <tbody>
                            {d.providers.map((p) => (
                                <tr key={p.info.id} className="border-t align-top">
                                    <td className="py-2 pr-3">
                                        {p.info.name}
                                        <div className="text-xs text-muted-foreground">
                                            {p.info.id} · {p.info.tier.kind}
                                        </div>
                                    </td>
                                    <td className="py-2 pr-3 text-xs">
                                        {p.installed ? (
                                            <>
                                                {p.installed.path}
                                                <div className="text-muted-foreground">
                                                    {p.installed.version}
                                                </div>
                                            </>
                                        ) : (
                                            <span className="text-muted-foreground">
                                                not found ({p.info.binaries.join(', ')})
                                            </span>
                                        )}
                                    </td>
                                    <td className="py-2 pr-3">{p.live?.label ?? '—'}</td>
                                    <td className="py-2 pr-3 text-xs">
                                        {p.slots.map((s) => (
                                            <div key={s}>{s}</div>
                                        ))}
                                    </td>
                                    <td className="py-2 text-xs">
                                        {p.warnings.map((w) => (
                                            <div key={w.code + w.message}>
                                                [{w.code}] {w.message}
                                            </div>
                                        ))}
                                    </td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                </CardContent>
            </Card>
        </div>
    )
}
