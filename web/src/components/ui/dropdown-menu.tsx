'use client'

import { Menu as MenuPrimitive } from '@base-ui/react/menu'
import { cn } from '@/lib/ui'

function DropdownMenu({ ...props }: MenuPrimitive.Root.Props) {
    return <MenuPrimitive.Root data-slot="dropdown-menu" {...props} />
}

function DropdownMenuTrigger({ ...props }: MenuPrimitive.Trigger.Props) {
    return <MenuPrimitive.Trigger data-slot="dropdown-menu-trigger" {...props} />
}

function DropdownMenuContent({
    className,
    sideOffset = 8,
    align = 'end',
    ...props
}: MenuPrimitive.Popup.Props & {
    sideOffset?: number
    align?: 'start' | 'center' | 'end'
}) {
    return (
        <MenuPrimitive.Portal>
            <MenuPrimitive.Positioner
                data-slot="dropdown-menu-positioner"
                className="z-50"
                sideOffset={sideOffset}
                align={align}
            >
                <MenuPrimitive.Popup
                    data-slot="dropdown-menu-content"
                    className={cn(
                        'min-w-44 rounded-2xl border border-border bg-popover p-1 text-sm text-popover-foreground shadow-md transition duration-150 data-ending-style:scale-95 data-ending-style:opacity-0 data-starting-style:scale-95 data-starting-style:opacity-0',
                        className
                    )}
                    {...props}
                />
            </MenuPrimitive.Positioner>
        </MenuPrimitive.Portal>
    )
}

function DropdownMenuItem({
    className,
    variant = 'default',
    ...props
}: MenuPrimitive.Item.Props & { variant?: 'default' | 'destructive' }) {
    return (
        <MenuPrimitive.Item
            data-slot="dropdown-menu-item"
            data-variant={variant}
            className={cn(
                "flex cursor-default items-center gap-2 rounded-xl px-3 py-2 outline-hidden select-none data-disabled:pointer-events-none data-disabled:opacity-50 data-highlighted:bg-accent data-highlighted:text-accent-foreground [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
                variant === 'destructive' &&
                    'text-destructive data-highlighted:bg-destructive/10 data-highlighted:text-destructive',
                className
            )}
            {...props}
        />
    )
}

export { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger }
