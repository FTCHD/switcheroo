import { cn } from '@/lib/ui'

const codes: Record<string, string> = {
    'claude-code': 'CC',
    codex: 'CX',
    'github-cli': 'GH',
    vercel: 'VC',
    wrangler: 'CF',
    npm: 'npm',
    fly: 'FLY',
    netlify: 'NT',
    'gemini-cli': 'GM',
    turso: 'TS',
    expo: 'EX',
    railway: 'RW',
}

export function Monogram({
    id,
    muted = false,
    size = 'md',
}: {
    id: string
    muted?: boolean
    size?: 'sm' | 'md'
}) {
    const code = codes[id] ?? id.slice(0, 2).toUpperCase()
    return (
        <span
            aria-hidden="true"
            className={cn(
                size === 'sm'
                    ? 'flex size-7 shrink-0 items-center justify-center rounded-full font-heading text-[10px] font-semibold tracking-[0.06em] select-none'
                    : 'flex size-11 shrink-0 items-center justify-center rounded-2xl font-heading text-[12px] font-semibold tracking-[0.06em] select-none',
                muted
                    ? 'bg-muted/60 text-muted-foreground/60 dark:bg-white/4'
                    : 'bg-muted text-muted-foreground dark:bg-white/8 dark:text-foreground/70'
            )}
        >
            {code}
        </span>
    )
}
