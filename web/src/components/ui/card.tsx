import type * as React from 'react'
import { cn } from '@/lib/ui'

function Card({ className, ...props }: React.ComponentProps<'div'>) {
    return (
        <div
            data-slot="card"
            className={cn(
                'flex flex-col gap-4 rounded-3xl border border-border bg-card p-6 text-card-foreground',
                className
            )}
            {...props}
        />
    )
}

function CardHeader({ className, ...props }: React.ComponentProps<'div'>) {
    return (
        <div data-slot="card-header" className={cn('flex flex-col gap-1', className)} {...props} />
    )
}

function CardTitle({ className, ...props }: React.ComponentProps<'div'>) {
    return (
        <div
            data-slot="card-title"
            className={cn('font-heading text-lg leading-none font-semibold', className)}
            {...props}
        />
    )
}

function CardDescription({ className, ...props }: React.ComponentProps<'div'>) {
    return (
        <div
            data-slot="card-description"
            className={cn('text-sm text-muted-foreground', className)}
            {...props}
        />
    )
}

function CardContent({ className, ...props }: React.ComponentProps<'div'>) {
    return <div data-slot="card-content" className={cn(className)} {...props} />
}

function CardFooter({ className, ...props }: React.ComponentProps<'div'>) {
    return (
        <div
            data-slot="card-footer"
            className={cn('flex items-center gap-2', className)}
            {...props}
        />
    )
}

export { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle }
