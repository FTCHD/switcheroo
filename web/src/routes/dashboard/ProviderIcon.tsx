import { TerminalIcon } from 'lucide-react'
import {
    type SimpleIcon,
    siClaude,
    siCloudflare,
    siExpo,
    siFlydotio,
    siGithub,
    siGooglegemini,
    siNetlify,
    siNpm,
    siRailway,
    siTurso,
    siVercel,
} from 'simple-icons'
import { cn } from '@/lib/ui'

// Brand marks from Simple Icons, drawn in currentColor so state (muted, active) comes from CSS.
// OpenAI's mark is not in the set, so Codex falls back to a terminal glyph.
const marks: Record<string, SimpleIcon> = {
    'claude-code': siClaude,
    'github-cli': siGithub,
    vercel: siVercel,
    wrangler: siCloudflare,
    npm: siNpm,
    fly: siFlydotio,
    netlify: siNetlify,
    'gemini-cli': siGooglegemini,
    turso: siTurso,
    expo: siExpo,
    railway: siRailway,
}

export function ProviderIcon({
    id,
    size = 'md',
    muted = false,
    className,
}: {
    id: string
    size?: 'sm' | 'md'
    muted?: boolean
    className?: string
}) {
    const mark = marks[id]
    return (
        <span
            aria-hidden="true"
            className={cn(
                'flex shrink-0 items-center justify-center select-none',
                size === 'sm' ? 'size-7 rounded-full' : 'size-11 rounded-2xl',
                muted
                    ? 'bg-muted/60 text-muted-foreground/50 dark:bg-white/4'
                    : 'bg-muted text-foreground/75 dark:bg-white/8 dark:text-foreground/80',
                className
            )}
        >
            {mark ? (
                <svg
                    viewBox="0 0 24 24"
                    fill="currentColor"
                    className={size === 'sm' ? 'size-3.5' : 'size-5'}
                >
                    <title>{mark.title}</title>
                    <path d={mark.path} />
                </svg>
            ) : (
                <TerminalIcon
                    strokeWidth={1.75}
                    className={size === 'sm' ? 'size-3.5' : 'size-5'}
                />
            )}
        </span>
    )
}
