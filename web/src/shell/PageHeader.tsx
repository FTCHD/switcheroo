import type { ReactNode } from 'react'

export function PageHeader({
    eyebrow,
    title,
    description,
    actions,
}: {
    eyebrow?: string
    title: string
    description?: ReactNode
    actions?: ReactNode
}) {
    return (
        <div className="mb-8 flex flex-wrap items-end justify-between gap-4">
            <div className="min-w-0">
                {eyebrow && (
                    <p className="mb-2 font-heading text-[12px] font-semibold tracking-[0.1em] text-muted-foreground uppercase">
                        {eyebrow}
                    </p>
                )}
                <h1 className="font-heading text-[32px] leading-[1.05] font-semibold tracking-[-0.02em] text-balance">
                    {title}
                </h1>
                {description && (
                    <p className="mt-3 max-w-xl text-[15px] leading-relaxed text-muted-foreground text-pretty">
                        {description}
                    </p>
                )}
            </div>
            {actions && <div className="flex shrink-0 items-center gap-2">{actions}</div>}
        </div>
    )
}
