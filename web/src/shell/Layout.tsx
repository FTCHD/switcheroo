import { NavLink, Outlet } from 'react-router'
import { useServerEvents } from '@/api/events'
import { boot } from '@/boot'
import { Toaster } from '@/components/ui/sonner'
import { cn } from '@/lib/utils'

const nav = [
    { to: '/', label: 'Accounts', end: true },
    { to: '/doctor', label: 'Doctor' },
    { to: '/settings', label: 'Settings' },
]

export function Layout() {
    useServerEvents()
    return (
        <div className="min-h-screen">
            <header className="border-b">
                <div className="mx-auto flex max-w-6xl items-center gap-6 px-6 py-3">
                    <NavLink to="/" className="flex items-center gap-2 font-semibold">
                        <span className="text-xl">🔀</span> Switcheroo
                    </NavLink>
                    <nav className="flex gap-1">
                        {nav.map((n) => (
                            <NavLink
                                key={n.to}
                                to={n.to}
                                end={n.end}
                                className={({ isActive }) =>
                                    cn(
                                        'rounded-md px-3 py-1.5 text-sm transition-colors hover:bg-accent',
                                        isActive ? 'bg-accent font-medium' : 'text-muted-foreground'
                                    )
                                }
                            >
                                {n.label}
                            </NavLink>
                        ))}
                    </nav>
                    <div className="ml-auto text-xs text-muted-foreground">
                        v{boot.version} · {boot.os} · vault: {boot.vault || 'n/a'}
                    </div>
                </div>
            </header>
            <main className="mx-auto max-w-6xl px-6 py-6">
                <Outlet />
            </main>
            <Toaster richColors position="bottom-right" />
        </div>
    )
}
