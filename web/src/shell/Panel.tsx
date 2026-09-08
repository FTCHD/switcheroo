import type { ReactNode } from 'react'
import { cn } from '@/lib/ui'

/** The one container shape the app uses: a soft elevated surface with hairline rows inside. */
export function Panel({
    eyebrow,
    className,
    children,
}: {
    eyebrow?: string
    className?: string
    children: ReactNode
}) {
    return (
        <section className={cn('mb-8', className)}>
            {eyebrow && (
                <h2 className="mb-3 px-1 font-heading text-[12px] font-semibold tracking-[0.1em] text-muted-foreground uppercase">
                    {eyebrow}
                </h2>
            )}
            <div className="panel divide-y divide-border/70 overflow-hidden">{children}</div>
        </section>
    )
}

/** A label/description on the left, a control on the right. */
export function PanelRow({
    label,
    description,
    children,
    htmlFor,
}: {
    label: string
    description?: ReactNode
    children: ReactNode
    htmlFor?: string
}) {
    return (
        <div className="flex flex-wrap items-center justify-between gap-x-8 gap-y-3 px-6 py-4">
            <div className="min-w-0 max-w-md">
                <label htmlFor={htmlFor} className="text-sm font-medium">
                    {label}
                </label>
                {description && (
                    <p className="mt-0.5 text-[13px] leading-snug text-muted-foreground text-pretty">
                        {description}
                    </p>
                )}
            </div>
            <div className="flex shrink-0 items-center gap-2">{children}</div>
        </div>
    )
}
