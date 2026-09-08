import { useEffect, useState } from 'react'
import { useProviders, useSaveSettings, useSettings } from '@/api/queries'
import type { Settings } from '@/api/types'
import { boot } from '@/boot'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import {
    Select,
    SelectContent,
    SelectItem,
    SelectTrigger,
    SelectValue,
} from '@/components/ui/select'
import { Skeleton } from '@/components/ui/skeleton'
import { Switch } from '@/components/ui/switch'
import { vaultName } from '@/lib/labels'
import { PageHeader } from '@/shell/PageHeader'
import { Panel, PanelRow } from '@/shell/Panel'

const vaultOptions: Record<Settings['vault'], string> = {
    auto: 'OS credential store (auto)',
    keychain: 'OS credential store',
    file: 'Plain file (0600)',
}

export function SettingsPage() {
    const q = useSettings()
    const providers = useProviders()
    const save = useSaveSettings()
    const [draft, setDraft] = useState<Settings | null>(null)
    useEffect(() => {
        if (q.data && !draft) setDraft(q.data)
    }, [q.data, draft])

    const dirty = draft && q.data && JSON.stringify(draft) !== JSON.stringify(q.data)

    const toggleHidden = (id: string, hidden: boolean) =>
        draft &&
        setDraft({
            ...draft,
            hidden_providers: hidden
                ? [...new Set([...draft.hidden_providers, id])]
                : draft.hidden_providers.filter((x) => x !== id),
        })

    return (
        <>
            <PageHeader
                title="Settings"
                description="How Switcheroo stores secrets, how it switches, and which CLIs it shows."
                actions={
                    draft && (
                        <Button
                            onClick={() => save.mutate(draft)}
                            disabled={!dirty || save.isPending}
                        >
                            {save.isPending ? 'Saving…' : 'Save changes'}
                        </Button>
                    )
                }
            />

            {!draft && (
                <Panel>
                    {[0, 1, 2].map((i) => (
                        <div key={i} className="px-6 py-4">
                            <Skeleton className="h-4 w-1/3" />
                        </div>
                    ))}
                </Panel>
            )}

            {draft && (
                <>
                    <Panel eyebrow="Secrets">
                        <PanelRow
                            label="Where remembered credentials go"
                            description={
                                <>
                                    Currently{' '}
                                    <b className="font-medium text-foreground">
                                        {vaultName(boot.vault)}
                                    </b>
                                    . A change takes effect on the next start; existing entries are
                                    not moved.
                                </>
                            }
                            htmlFor="vault"
                        >
                            <Select
                                value={draft.vault}
                                items={vaultOptions}
                                onValueChange={(v) =>
                                    setDraft({ ...draft, vault: v as Settings['vault'] })
                                }
                            >
                                <SelectTrigger id="vault" className="w-60">
                                    <SelectValue />
                                </SelectTrigger>
                                <SelectContent>
                                    {Object.entries(vaultOptions).map(([value, label]) => (
                                        <SelectItem key={value} value={value}>
                                            {label}
                                        </SelectItem>
                                    ))}
                                </SelectContent>
                            </Select>
                        </PanelRow>
                        <PanelRow
                            label="State directory"
                            description="Labels, timestamps and the cached identity per CLI. Never secrets."
                        >
                            <code className="font-mono text-[12px] text-muted-foreground break-all">
                                {boot.dataDir}
                            </code>
                        </PanelRow>
                    </Panel>

                    <Panel eyebrow="Switching">
                        <PanelRow
                            label="Ask before switching"
                            description="Show a confirmation in the web UI before a login is replaced."
                            htmlFor="confirm"
                        >
                            <Switch
                                id="confirm"
                                checked={draft.tray.confirm_switch}
                                onCheckedChange={(v) =>
                                    setDraft({
                                        ...draft,
                                        tray: { ...draft.tray, confirm_switch: v },
                                    })
                                }
                            />
                        </PanelRow>
                        <PanelRow
                            label="Show emails in the tray menu"
                            description="Off shows only the labels you gave accounts."
                            htmlFor="emails"
                        >
                            <Switch
                                id="emails"
                                checked={draft.tray.show_emails}
                                onCheckedChange={(v) =>
                                    setDraft({ ...draft, tray: { ...draft.tray, show_emails: v } })
                                }
                            />
                        </PanelRow>
                    </Panel>

                    <Panel eyebrow="Web UI">
                        <PanelRow
                            label="Listen address"
                            description="Loopback only. Leave blank for a port that stays the same across restarts."
                            htmlFor="bind"
                        >
                            <Input
                                id="bind"
                                placeholder="127.0.0.1:20123"
                                className="w-56 font-mono text-base sm:text-sm"
                                value={draft.bind ?? ''}
                                onChange={(e) =>
                                    setDraft({ ...draft, bind: e.target.value || null })
                                }
                            />
                        </PanelRow>
                    </Panel>

                    <Panel eyebrow="CLIs shown">
                        {(providers.data ?? []).map((p) => (
                            <PanelRow
                                key={p.info.id}
                                label={p.info.name}
                                description={p.installed ? p.installed.path : 'Not on this machine'}
                                htmlFor={`show-${p.info.id}`}
                            >
                                <Switch
                                    id={`show-${p.info.id}`}
                                    checked={!draft.hidden_providers.includes(p.info.id)}
                                    onCheckedChange={(v) => toggleHidden(p.info.id, !v)}
                                />
                            </PanelRow>
                        ))}
                    </Panel>
                </>
            )}
        </>
    )
}
