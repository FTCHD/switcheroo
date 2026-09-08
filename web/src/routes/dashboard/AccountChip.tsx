import { CheckIcon, ChevronDownIcon } from 'lucide-react'
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
import { Button } from '@/components/ui/button'
import {
    Dialog,
    DialogContent,
    DialogDescription,
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
import { Spinner } from '@/components/ui/spinner'
import { cn } from '@/lib/ui'

/**
 * One remembered account, drawn as a jack on the switchboard: filled when it is the live
 * login, outlined otherwise. Click to switch; the caret opens rename/forget.
 */
export function AccountChip({
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
    const [forgetting, setForgetting] = useState(false)
    const pending = sw.isPending

    const doSwitch = () => sw.mutate({ provider: account.provider, account: account.id })

    return (
        <>
            <div
                data-active={active}
                className={cn(
                    'group/chip flex h-9 items-stretch overflow-hidden rounded-full border text-sm transition-colors duration-150',
                    active
                        ? 'border-primary bg-primary text-primary-foreground'
                        : 'border-border bg-background text-foreground hover:bg-muted dark:bg-transparent dark:hover:bg-input/30'
                )}
            >
                <button
                    type="button"
                    title={account.id}
                    disabled={active || pending}
                    onClick={() => (confirmSwitch ? setConfirming(true) : doSwitch())}
                    className={cn(
                        'flex items-center gap-1.5 pr-1.5 pl-3 font-medium outline-none transition-transform duration-150 select-none focus-visible:ring-3 focus-visible:ring-ring/30',
                        active ? 'cursor-default' : 'active:scale-[0.96] disabled:opacity-60'
                    )}
                >
                    {pending ? (
                        <Spinner className="size-3.5" />
                    ) : active ? (
                        <CheckIcon className="size-3.5" strokeWidth={2.5} />
                    ) : null}
                    <span className="max-w-[18rem] truncate">{account.label}</span>
                    {!account.has_secret && (
                        <span className="text-[11px] font-normal opacity-70">native</span>
                    )}
                </button>
                <DropdownMenu>
                    <DropdownMenuTrigger
                        aria-label={`More for ${account.label}`}
                        className={cn(
                            'flex items-center pr-2 pl-0.5 opacity-55 outline-none transition-opacity duration-150 group-hover/chip:opacity-100 focus-visible:opacity-100 aria-expanded:opacity-100'
                        )}
                    >
                        <ChevronDownIcon className="size-3.5" strokeWidth={2} />
                    </DropdownMenuTrigger>
                    <DropdownMenuContent align="start">
                        <DropdownMenuItem onClick={() => setRenaming(true)}>
                            Rename
                        </DropdownMenuItem>
                        <DropdownMenuItem variant="destructive" onClick={() => setForgetting(true)}>
                            Forget
                        </DropdownMenuItem>
                    </DropdownMenuContent>
                </DropdownMenu>
            </div>

            <Dialog open={renaming} onOpenChange={setRenaming}>
                <DialogContent>
                    <DialogHeader>
                        <DialogTitle>Rename account</DialogTitle>
                        <DialogDescription className="font-mono text-[13px] break-all">
                            {account.id}
                        </DialogDescription>
                    </DialogHeader>
                    <form
                        className="contents"
                        onSubmit={(e) => {
                            e.preventDefault()
                            rename.mutate({
                                provider: account.provider,
                                account: account.id,
                                label: label.trim() || account.id,
                            })
                            setRenaming(false)
                        }}
                    >
                        <Input
                            value={label}
                            onChange={(e) => setLabel(e.target.value)}
                            autoFocus
                            placeholder="Work, personal, client…"
                            className="text-base sm:text-sm"
                        />
                        <DialogFooter>
                            <Button
                                type="button"
                                variant="outline"
                                onClick={() => setRenaming(false)}
                            >
                                Cancel
                            </Button>
                            <Button type="submit">Save name</Button>
                        </DialogFooter>
                    </form>
                </DialogContent>
            </Dialog>

            <AlertDialog open={confirming} onOpenChange={setConfirming}>
                <AlertDialogContent>
                    <AlertDialogHeader>
                        <AlertDialogTitle>Switch to {account.label}?</AlertDialogTitle>
                        <AlertDialogDescription>
                            The current login is remembered first, then replaced. Every terminal
                            using this CLI switches with it.
                        </AlertDialogDescription>
                    </AlertDialogHeader>
                    <AlertDialogFooter>
                        <AlertDialogCancel>Cancel</AlertDialogCancel>
                        <AlertDialogAction onClick={doSwitch}>Switch</AlertDialogAction>
                    </AlertDialogFooter>
                </AlertDialogContent>
            </AlertDialog>

            <AlertDialog open={forgetting} onOpenChange={setForgetting}>
                <AlertDialogContent>
                    <AlertDialogHeader>
                        <AlertDialogTitle>Forget {account.label}?</AlertDialogTitle>
                        <AlertDialogDescription>
                            {account.has_secret
                                ? 'The remembered credential is deleted from your credential store. The CLI itself is not logged out.'
                                : 'Only the label is removed; the CLI keeps this account.'}
                        </AlertDialogDescription>
                    </AlertDialogHeader>
                    <AlertDialogFooter>
                        <AlertDialogCancel>Keep</AlertDialogCancel>
                        <AlertDialogAction
                            variant="destructive"
                            onClick={() =>
                                remove.mutate({ provider: account.provider, account: account.id })
                            }
                        >
                            Forget
                        </AlertDialogAction>
                    </AlertDialogFooter>
                </AlertDialogContent>
            </AlertDialog>
        </>
    )
}
