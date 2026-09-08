// Small formatting helpers shared by the pages.

export function humanizeUntil(iso: string, now = Date.now()): string {
    const secs = Math.floor((new Date(iso).getTime() - now) / 1000)
    if (!Number.isFinite(secs) || secs <= 0) return 'now'
    const d = Math.floor(secs / 86_400)
    const h = Math.floor((secs % 86_400) / 3600)
    const m = Math.floor((secs % 3600) / 60)
    if (d > 0) return `${d}d ${h}h`
    if (h > 0) return `${h}h ${m}m`
    return `${Math.max(m, 1)}m`
}

export function humanizeAgo(iso: string, now = Date.now()): string {
    const secs = Math.floor((now - new Date(iso).getTime()) / 1000)
    if (secs < 45) return 'just now'
    if (secs < 3600) return `${Math.round(secs / 60)} min ago`
    return `${Math.round(secs / 3600)} h ago`
}

export const thousands = new Intl.NumberFormat('en-US', { maximumFractionDigits: 0 })
