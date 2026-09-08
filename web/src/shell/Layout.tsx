import { MoonIcon, SunIcon } from 'lucide-react'
import { NavLink, Outlet } from 'react-router'
import { useServerEvents } from '@/api/events'
import { boot } from '@/boot'
import { Button } from '@/components/ui/button'
import { useTheme } from '@/hooks/use-theme'
import { cn } from '@/lib/ui'

const nav = [
    { to: '/', label: 'Accounts', end: true },
    { to: '/doctor', label: 'Doctor' },
    { to: '/settings', label: 'Settings' },
]

function ThemeToggle() {
    const { theme, setTheme } = useTheme()
    const dark =
        theme === 'dark' ||
        (theme === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches)
    return (
        <Button
            variant="ghost"
            size="icon-sm"
            aria-label="Toggle theme"
            onClick={() => setTheme(dark ? 'light' : 'dark')}
        >
            {dark ? <SunIcon /> : <MoonIcon />}
        </Button>
    )
}

export function Layout() {
    useServerEvents()
    return (
        <div className="min-h-screen">
            <header className="border-b">
                <div className="mx-auto flex max-w-6xl items-center gap-6 px-6 py-3">
                    <NavLink to="/" className="font-heading text-lg font-semibold">
                        Switcheroo
                    </NavLink>
                    <nav className="flex gap-1">
                        {nav.map((n) => (
                            <NavLink
                                key={n.to}
                                to={n.to}
                                end={n.end}
                                className={({ isActive }) =>
                                    cn(
                                        'rounded-4xl px-3 py-1.5 text-sm transition-colors hover:bg-muted',
                                        isActive ? 'bg-muted font-medium' : 'text-muted-foreground'
                                    )
                                }
                            >
                                {n.label}
                            </NavLink>
                        ))}
                    </nav>
                    <div className="ml-auto flex items-center gap-3 text-xs text-muted-foreground">
                        <span>
                            v{boot.version} · {boot.os} · vault: {boot.vault || 'n/a'}
                        </span>
                        <ThemeToggle />
                    </div>
                </div>
            </header>
            <main className="mx-auto max-w-6xl px-6 py-6">
                <Outlet />
            </main>
        </div>
    )
}
