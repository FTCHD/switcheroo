// Server-sent events → query invalidation, so CLI/tray-driven changes show up live.

import { useQueryClient } from '@tanstack/react-query'
import { useEffect } from 'react'
import { keys } from './queries'

export function useServerEvents() {
    const qc = useQueryClient()
    useEffect(() => {
        const es = new EventSource('/api/events')
        const refresh = () => {
            qc.invalidateQueries({ queryKey: keys.providers })
        }
        for (const kind of [
            'state.changed',
            'switch.done',
            'account.saved',
            'account.removed',
            'account.renamed',
        ]) {
            es.addEventListener(kind, refresh)
        }
        es.addEventListener('settings.changed', () => {
            qc.invalidateQueries({ queryKey: keys.settings })
        })
        es.addEventListener('autostart.changed', () => {
            qc.invalidateQueries({ queryKey: keys.autostart })
        })
        return () => es.close()
    }, [qc])
}
