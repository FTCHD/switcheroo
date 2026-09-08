import { RefreshCwIcon, TriangleAlertIcon } from 'lucide-react'
import { useRefreshUsage, useUsage } from '@/api/queries'
import type { UsageItem } from '@/api/types'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { humanizeAgo, humanizeUntil, thousands } from '@/lib/format'
import { cn } from '@/lib/ui'

function Meter({ item, color }: { item: UsageItem; color?: string }) {
    if (item.kind === 'text') {
        return (
            <div className="min-w-[6rem]">
                <div className="text-[13px] text-muted-foreground">{item.label}</div>
                <div className="mt-1.5 flex h-1.5 items-center">
                    <span className="text-[13px] leading-none font-medium">{item.value}</span>
                </div>
                <div className="mt-1 h-4 text-[12px] text-muted-foreground">{item.detail}</div>
            </div>
        )
    }
    const pct = item.kind === 'percent' ? item.used : (item.used / Math.max(item.limit, 1)) * 100
    const value =
        item.kind === 'percent'
            ? `${Math.round(item.used)}%`
            : `${thousands.format(item.used)} / ${thousands.format(item.limit)}${item.unit ? ` ${item.unit}` : ''}`
    const hot = pct >= 90
    return (
        <div className="min-w-[10rem] flex-1 basis-40">
            <div className="flex items-baseline justify-between gap-3 text-[13px]">
                <span className="truncate text-muted-foreground">{item.label}</span>
                <span className={cn('font-medium tabular-nums', hot && 'text-destructive')}>
                    {value}
                </span>
            </div>
            <div className="mt-1.5 h-1.5 overflow-hidden rounded-full bg-muted">
                <div
                    className={cn(
                        'h-full rounded-full transition-[width] duration-500',
                        hot ? 'bg-destructive' : 'bg-primary'
                    )}
                    style={{
                        width: `${Math.min(100, Math.max(0, pct))}%`,
                        ...(!hot && color ? { backgroundColor: color } : {}),
                    }}
                />
            </div>
            <div className="mt-1 h-4 text-[12px] text-muted-foreground tabular-nums">
                {item.resets_at ? `resets in ${humanizeUntil(item.resets_at)}` : item.detail}
            </div>
        </div>
    )
}

/**
 * Usage meters for one signed-in provider. The provider decides what the items are; this only
 * knows how to draw a percent, a gauge, or a line of text.
 */
export function UsageStrip({
    providerId,
    color,
    enabled,
    expanded = false,
}: {
    providerId: string
    color?: string
    enabled: boolean
    expanded?: boolean
}) {
    const q = useUsage(providerId, enabled)
    const refresh = useRefreshUsage(providerId)
    if (!enabled) return null
    if (q.isLoading) {
        return (
            <div className="mt-4 flex gap-6">
                <Skeleton className="h-9 w-40" />
                <Skeleton className="h-9 w-40" />
            </div>
        )
    }
    if (q.error) {
        return (
            <p className="mt-3 flex items-start gap-2 text-[13px] text-muted-foreground text-pretty">
                <TriangleAlertIcon
                    className="mt-px size-3.5 shrink-0 text-destructive"
                    strokeWidth={2}
                />
                Usage unavailable: {q.error.message}
            </p>
        )
    }
    if (!q.data || q.data.items.length === 0) return null
    return (
        <div className="mt-4">
            <div className="flex flex-wrap items-start gap-x-8 gap-y-3">
                {q.data.items.map((item) => (
                    <Meter key={item.label} item={item} color={color} />
                ))}
            </div>
            {expanded && (
                <div className="mt-3 flex items-center gap-2 text-[12px] text-muted-foreground">
                    <span className="text-pretty">{q.data.note}</span>
                    <span className="ml-auto shrink-0 tabular-nums">
                        checked {humanizeAgo(q.data.fetched_at)}
                    </span>
                    <Button
                        size="icon-xs"
                        variant="ghost"
                        aria-label="Refresh usage"
                        onClick={() => refresh.mutate()}
                        disabled={refresh.isPending}
                    >
                        <RefreshCwIcon
                            strokeWidth={1.75}
                            className={refresh.isPending ? 'animate-spin' : ''}
                        />
                    </Button>
                </div>
            )}
        </div>
    )
}
