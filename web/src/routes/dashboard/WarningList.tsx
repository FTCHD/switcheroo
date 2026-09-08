import { AlertTriangle, Info } from 'lucide-react'
import type { Warning } from '@/api/types'
import { Alert, AlertDescription } from '@/components/ui/alert'

export function WarningList({ warnings }: { warnings: Warning[] }) {
    if (warnings.length === 0) return null
    return (
        <div className="space-y-2">
            {warnings.map((w) => (
                <Alert
                    key={w.code + w.message}
                    variant={w.severity === 'warn' ? 'destructive' : 'default'}
                    className="py-2"
                >
                    {w.severity === 'warn' ? (
                        <AlertTriangle className="size-4" />
                    ) : (
                        <Info className="size-4" />
                    )}
                    <AlertDescription className="text-xs">{w.message}</AlertDescription>
                </Alert>
            ))}
        </div>
    )
}
