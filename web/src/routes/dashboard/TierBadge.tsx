import type { TierInfo } from '@/api/types'
import { Badge } from '@/components/ui/badge'

export function TierBadge({ tier }: { tier: TierInfo }) {
    if (tier.kind === 'supported') return null
    if (tier.kind === 'experimental') return <Badge variant="outline">experimental</Badge>
    return (
        <Badge variant="destructive" title={tier.reason}>
            unsupported
        </Badge>
    )
}
