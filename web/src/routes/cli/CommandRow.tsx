import { CheckIcon, CopyIcon } from 'lucide-react'
import type { ReactNode } from 'react'
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { toast } from '@/components/ui/toast'

/** One command: the invocation in mono on the left, what it does on the right, copy on hover. */
export function CommandRow({
    command,
    example,
    children,
}: {
    command: string
    example?: string
    children: ReactNode
}) {
    const [copied, setCopied] = useState(false)
    const text = example ?? command
    const copy = async () => {
        try {
            await navigator.clipboard.writeText(text)
            setCopied(true)
            setTimeout(() => setCopied(false), 1500)
        } catch {
            toast.add({ title: 'Could not copy', description: text, type: 'error' })
        }
    }
    return (
        <div className="group/cmd grid gap-x-8 gap-y-2 px-6 py-4 sm:grid-cols-[minmax(0,22rem)_1fr]">
            <div className="min-w-0">
                <code className="block font-mono text-[13px] leading-relaxed break-words text-foreground">
                    {command}
                </code>
                {example && (
                    <div className="mt-1.5 flex items-start gap-1.5">
                        <code className="min-w-0 rounded-md bg-muted px-1.5 py-0.5 font-mono text-[12px] leading-relaxed break-words text-muted-foreground">
                            {example}
                        </code>
                        <Button
                            size="icon-xs"
                            variant="ghost"
                            aria-label={copied ? 'Copied' : `Copy ${example}`}
                            className="shrink-0 text-muted-foreground opacity-0 transition-opacity duration-150 group-hover/cmd:opacity-100 focus-visible:opacity-100"
                            onClick={copy}
                        >
                            {copied ? (
                                <CheckIcon className="text-primary" strokeWidth={2.5} />
                            ) : (
                                <CopyIcon strokeWidth={1.75} />
                            )}
                        </Button>
                    </div>
                )}
            </div>
            <div className="text-sm leading-relaxed text-muted-foreground text-pretty [&_code]:rounded [&_code]:bg-muted [&_code]:px-1 [&_code]:py-0.5 [&_code]:font-mono [&_code]:text-[12px] [&_code]:text-foreground/80">
                {children}
            </div>
        </div>
    )
}
