import { MoonIcon, SunIcon } from 'lucide-react'
import { NavLink, Outlet } from 'react-router'
import { useServerEvents } from '@/api/events'
import { boot } from '@/boot'
import { Button } from '@/components/ui/button'
import { useTheme } from '@/hooks/use-theme'
import { vaultName } from '@/lib/labels'
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
            aria-label={dark ? 'Switch to light theme' : 'Switch to dark theme'}
            onClick={() => setTheme(dark ? 'light' : 'dark')}
        >
            {dark ? <SunIcon strokeWidth={1.75} /> : <MoonIcon strokeWidth={1.75} />}
        </Button>
    )
}

export function Layout() {
    useServerEvents()
    return (
        <div className="min-h-screen">
            <header className="sticky top-0 z-30 border-b border-border/60 bg-background/80 backdrop-blur-md">
                <div className="mx-auto flex h-14 max-w-5xl items-center gap-8 px-6">
                    <NavLink
                        to="/"
                        className="flex items-center gap-2.5 font-heading text-[17px] font-semibold tracking-[-0.01em]"
                    >
                        <span aria-hidden="true" className="relative flex size-2.5">
                            <span className="absolute inset-0 rounded-full bg-primary motion-safe:animate-live" />
                            <span className="relative size-2.5 rounded-full bg-primary" />
                        </span>
                        Switcheroo
                    </NavLink>
                    <nav className="flex items-center gap-0.5" aria-label="Main">
                        {nav.map((n) => (
                            <NavLink
                                key={n.to}
                                to={n.to}
                                end={n.end}
                                className={({ isActive }) =>
                                    cn(
                                        'rounded-4xl px-3 py-1.5 text-sm transition-colors duration-150 hover:bg-muted hover:text-foreground',
                                        isActive
                                            ? 'bg-muted font-medium text-foreground'
                                            : 'text-muted-foreground'
                                    )
                                }
                            >
                                {n.label}
                            </NavLink>
                        ))}
                    </nav>
                    <div className="ml-auto flex items-center gap-3">
                        <span className="hidden font-mono text-[12px] text-muted-foreground sm:inline">
                            {boot.vault ? vaultName(boot.vault) : 'no vault'} · v{boot.version}
                        </span>
                        <ThemeToggle />
                    </div>
                </div>
            </header>
            <main className="mx-auto max-w-5xl px-6 py-10">
                <Outlet />
            </main>
        </div>
    )
}
