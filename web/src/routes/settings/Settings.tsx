import { useEffect, useState } from 'react'
import { useProviders, useSaveSettings, useSettings } from '@/api/queries'
import type { Settings } from '@/api/types'
import { boot } from '@/boot'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Switch } from '@/components/ui/switch'

export function SettingsPage() {
    const q = useSettings()
    const providers = useProviders()
    const save = useSaveSettings()
    const [draft, setDraft] = useState<Settings | null>(null)
    useEffect(() => {
        if (q.data && !draft) setDraft(q.data)
    }, [q.data, draft])
    if (!draft) return <p className="text-muted-foreground">Loading…</p>

    const toggleHidden = (id: string, hidden: boolean) =>
        setDraft({
            ...draft,
            hidden_providers: hidden
                ? [...new Set([...draft.hidden_providers, id])]
                : draft.hidden_providers.filter((x) => x !== id),
        })

    return (
        <div className="mx-auto max-w-2xl space-y-4">
            <Card>
                <CardHeader>
                    <CardTitle>Secrets</CardTitle>
                </CardHeader>
                <CardContent className="space-y-3 text-sm">
                    <p>
                        Saved credentials are stored in{' '}
                        <b>{boot.vault || 'the OS credential store'}</b>. Metadata lives in{' '}
                        <code>{boot.dataDir}</code>.
                    </p>
                    <div className="flex items-center gap-3">
                        <Label htmlFor="vault">Backend</Label>
                        <select
                            id="vault"
                            className="h-9 rounded-4xl border border-border bg-background px-3 text-sm"
                            value={draft.vault}
                            onChange={(e) =>
                                setDraft({ ...draft, vault: e.target.value as Settings['vault'] })
                            }
                        >
                            <option value="auto">OS credential store (auto)</option>
                            <option value="keychain">OS credential store</option>
                            <option value="file">Plaintext file (0600)</option>
                        </select>
                        <span className="text-xs text-muted-foreground">
                            Takes effect on the next start. Existing entries are not migrated.
                        </span>
                    </div>
                </CardContent>
            </Card>

            <Card>
                <CardHeader>
                    <CardTitle>Tray & switching</CardTitle>
                </CardHeader>
                <CardContent className="space-y-3">
                    <div className="flex items-center justify-between">
                        <Label htmlFor="emails">Show emails in the tray menu</Label>
                        <Switch
                            id="emails"
                            checked={draft.tray.show_emails}
                            onCheckedChange={(v) =>
                                setDraft({ ...draft, tray: { ...draft.tray, show_emails: v } })
                            }
                        />
                    </div>
                    <div className="flex items-center justify-between">
                        <Label htmlFor="confirm">Ask before switching</Label>
                        <Switch
                            id="confirm"
                            checked={draft.tray.confirm_switch}
                            onCheckedChange={(v) =>
                                setDraft({ ...draft, tray: { ...draft.tray, confirm_switch: v } })
                            }
                        />
                    </div>
                </CardContent>
            </Card>

            <Card>
                <CardHeader>
                    <CardTitle>Server</CardTitle>
                </CardHeader>
                <CardContent className="space-y-2">
                    <Label htmlFor="bind">Listen address (loopback only; blank = automatic)</Label>
                    <Input
                        id="bind"
                        placeholder="127.0.0.1:20123"
                        value={draft.bind ?? ''}
                        onChange={(e) => setDraft({ ...draft, bind: e.target.value || null })}
                    />
                </CardContent>
            </Card>

            <Card>
                <CardHeader>
                    <CardTitle>Hidden providers</CardTitle>
                </CardHeader>
                <CardContent className="grid gap-2 sm:grid-cols-2">
                    {(providers.data ?? []).map((p) => (
                        <div key={p.info.id} className="flex items-center justify-between">
                            <Label htmlFor={`hide-${p.info.id}`}>{p.info.name}</Label>
                            <Switch
                                id={`hide-${p.info.id}`}
                                checked={draft.hidden_providers.includes(p.info.id)}
                                onCheckedChange={(v) => toggleHidden(p.info.id, v)}
                            />
                        </div>
                    ))}
                </CardContent>
            </Card>

            <div className="flex justify-end">
                <Button onClick={() => save.mutate(draft)} disabled={save.isPending}>
                    Save settings
                </Button>
            </div>
        </div>
    )
}
