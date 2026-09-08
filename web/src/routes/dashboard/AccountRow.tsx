import { MoreHorizontalIcon } from 'lucide-react'
import { useState } from 'react'
import { useRemove, useRename, useSwitch } from '@/api/queries'
import type { Account } from '@/api/types'
import {
    AlertDialog,
    AlertDialogAction,
    AlertDialogCancel,
    AlertDialogContent,
    AlertDialogDescription,
    AlertDialogFooter,
    AlertDialogHeader,
    AlertDialogTitle,
} from '@/components/ui/alert-dialog'
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
        <div className="flex items-center gap-2 rounded-2xl border px-3 py-2">
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
                <DropdownMenuTrigger
                    render={<Button size="icon-sm" variant="ghost" aria-label="More" />}
                >
                    <MoreHorizontalIcon />
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end">
                    <DropdownMenuItem onClick={() => setRenaming(true)}>Rename</DropdownMenuItem>
                    <DropdownMenuItem
                        variant="destructive"
                        onClick={() =>
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

            <AlertDialog open={confirming} onOpenChange={setConfirming}>
                <AlertDialogContent>
                    <AlertDialogHeader>
                        <AlertDialogTitle>Switch to {account.label}?</AlertDialogTitle>
                        <AlertDialogDescription>
                            The current login is saved first, then replaced. Every terminal using
                            this CLI is affected.
                        </AlertDialogDescription>
                    </AlertDialogHeader>
                    <AlertDialogFooter>
                        <AlertDialogCancel>Cancel</AlertDialogCancel>
                        <AlertDialogAction onClick={doSwitch}>Switch</AlertDialogAction>
                    </AlertDialogFooter>
                </AlertDialogContent>
            </AlertDialog>
        </div>
    )
}
