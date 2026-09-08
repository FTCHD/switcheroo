import { InfoIcon, TriangleAlertIcon } from 'lucide-react'
import type { Warning } from '@/api/types'
import { cn } from '@/lib/ui'

export function WarningList({ warnings, className }: { warnings: Warning[]; className?: string }) {
    if (warnings.length === 0) return null
    return (
        <ul className={cn('space-y-1.5', className)}>
            {warnings.map((w) => {
                const warn = w.severity === 'warn'
                const Icon = warn ? TriangleAlertIcon : InfoIcon
                return (
                    <li
                        key={w.code + w.message}
                        className="flex items-start gap-2 text-[13px] leading-snug text-muted-foreground text-pretty"
                    >
                        <Icon
                            className={cn('mt-px size-3.5 shrink-0', warn && 'text-destructive')}
                            strokeWidth={2}
                        />
                        <span>{w.message}</span>
                    </li>
                )
            })}
        </ul>
    )
}
