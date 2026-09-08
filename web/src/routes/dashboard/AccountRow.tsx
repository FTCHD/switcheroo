import { MoreHorizontal } from 'lucide-react'
import { useState } from 'react'
import { useRemove, useRename, useSwitch } from '@/api/queries'
import type { Account } from '@/api/types'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
    Dialog,
    DialogContent,
    DialogFooter,
    DialogHeader,
    DialogTitle,
} from '@/components/ui/dialog'
import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { Input } from '@/components/ui/input'

export function AccountRow({
    account,
    active,
    confirmSwitch,
}: {
    account: Account
    active: boolean
    confirmSwitch: boolean
}) {
    const sw = useSwitch()
    const remove = useRemove()
    const rename = useRename()
    const [renaming, setRenaming] = useState(false)
    const [label, setLabel] = useState(account.label)
    const [confirming, setConfirming] = useState(false)

    const doSwitch = () => sw.mutate({ provider: account.provider, account: account.id })

    return (
        <div className="flex items-center gap-2 rounded-md border px-3 py-2">
            <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                    <span className="truncate text-sm font-medium">{account.label}</span>
                    {active && <Badge>active</Badge>}
                    {!account.has_secret && (
                        <Badge variant="secondary" title="Held by the CLI itself">
                            native
                        </Badge>
                    )}
                </div>
                {account.label !== account.id && (
                    <div className="truncate text-xs text-muted-foreground">{account.id}</div>
                )}
            </div>
            <Button
                size="sm"
                variant={active ? 'secondary' : 'default'}
                disabled={active || sw.isPending}
                onClick={() => (confirmSwitch ? setConfirming(true) : doSwitch())}
            >
                {sw.isPending ? 'Switching…' : active ? 'Active' : 'Switch'}
            </Button>
            <DropdownMenu>
                <DropdownMenuTrigger asChild>
                    <Button size="icon" variant="ghost" aria-label="More">
                        <MoreHorizontal className="size-4" />
                    </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end">
                    <DropdownMenuItem onSelect={() => setRenaming(true)}>Rename</DropdownMenuItem>
                    <DropdownMenuItem
                        className="text-destructive"
                        onSelect={() =>
                            remove.mutate({ provider: account.provider, account: account.id })
                        }
                    >
                        Remove
                    </DropdownMenuItem>
                </DropdownMenuContent>
            </DropdownMenu>

            <Dialog open={renaming} onOpenChange={setRenaming}>
                <DialogContent>
                    <DialogHeader>
                        <DialogTitle>Rename {account.id}</DialogTitle>
                    </DialogHeader>
                    <Input value={label} onChange={(e) => setLabel(e.target.value)} autoFocus />
                    <DialogFooter>
                        <Button variant="outline" onClick={() => setRenaming(false)}>
                            Cancel
                        </Button>
                        <Button
                            onClick={() => {
                                rename.mutate({
                                    provider: account.provider,
                                    account: account.id,
                                    label,
                                })
                                setRenaming(false)
                            }}
                        >
                            Save
                        </Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>

            <Dialog open={confirming} onOpenChange={setConfirming}>
                <DialogContent>
                    <DialogHeader>
                        <DialogTitle>Switch to {account.label}?</DialogTitle>
                    </DialogHeader>
                    <p className="text-sm text-muted-foreground">
                        The current login is saved first, then replaced. Every terminal using this
                        CLI is affected.
                    </p>
                    <DialogFooter>
                        <Button variant="outline" onClick={() => setConfirming(false)}>
                            Cancel
                        </Button>
                        <Button
                            onClick={() => {
                                setConfirming(false)
                                doSwitch()
                            }}
                        >
                            Switch
                        </Button>
                    </DialogFooter>
                </DialogContent>
            </Dialog>
        </div>
    )
}
