import { useContext } from 'react'
import { invariant } from '@/lib/ui'
import { ThemeProviderContext } from '../components/theme-provider'

export function useTheme() {
    const context = useContext(ThemeProviderContext)
    invariant(context, 'useTheme must be used within a ThemeProvider')
    return context
}
