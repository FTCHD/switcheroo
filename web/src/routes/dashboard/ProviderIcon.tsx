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
    color,
    size = 'md',
    muted = false,
    className,
}: {
    id: string
    /** Provider accent; the mark is drawn in it. Falls back to the text color. */
    color?: string
    size?: 'sm' | 'md'
    muted?: boolean
    className?: string
}) {
    const mark = marks[id]
    return (
        <span
            aria-hidden="true"
            style={color ? { color } : undefined}
            className={cn(
                'flex shrink-0 items-center justify-center select-none transition-opacity duration-150',
                size === 'sm' ? 'size-7' : 'size-11',
                !color && (muted ? 'text-muted-foreground/45' : 'text-foreground/85'),
                color && muted && 'opacity-45',
                className
            )}
        >
            {mark ? (
                <svg
                    viewBox="0 0 24 24"
                    fill="currentColor"
                    className={size === 'sm' ? 'size-4' : 'size-6'}
                >
                    <title>{mark.title}</title>
                    <path d={mark.path} />
                </svg>
            ) : (
                <TerminalIcon strokeWidth={1.75} className={size === 'sm' ? 'size-4' : 'size-6'} />
            )}
        </span>
    )
}
