// Platform facts injected by the Rust server into index.html before the first render.
// Under the Vite dev server nothing is injected, so we fall back to the fixed dev token.

export interface Boot {
    session: string
    version: string
    os: string
    arch: string
    dev: boolean
    vault: string
    dataDir: string
}

declare global {
    interface Window {
        __SWITCHEROO__?: Boot
    }
}

export const boot: Boot = window.__SWITCHEROO__ ?? {
    session: 'dev',
    version: 'dev',
    os: 'unknown',
    arch: '',
    dev: true,
    vault: '',
    dataDir: '',
}
