// Mirrors src/core/model.rs. Keep in sync by hand; the shapes are small.

export type Strategy = 'slot-swap' | 'native-switch'

export type TierInfo =
    | { kind: 'supported' }
    | { kind: 'experimental' }
    | { kind: 'unsupported'; reason: string }

export interface ProviderInfo {
    id: string
    name: string
    strategy: Strategy
    tier: TierInfo
    binaries: string[]
    env_shadow: string[]
    restart_hint: string | null
    notes: string
    login_command: string[]
}

export interface Identity {
    id: string
    label: string
    email?: string
    extra?: Record<string, string>
}

export interface Installed {
    path: string
    version: string | null
}

export interface Warning {
    severity: 'info' | 'warn'
    code: string
    message: string
}

export interface Account {
    provider: string
    id: string
    label: string
    identity: Identity
    saved_at: string
    last_used: string | null
    has_secret: boolean
}

export interface ProviderStatus {
    info: ProviderInfo
    installed: Installed | null
    live: Identity | null
    active_account: string | null
    accounts: Account[]
    warnings: Warning[]
    slots: string[]
    live_checked_at: string | null
}

export interface SwitchOutcome {
    provider: string
    account: Account
    recaptured: string | null
    warnings: Warning[]
}

export interface Settings {
    vault: 'auto' | 'keychain' | 'file'
    bind: string | null
    tray: { show_emails: boolean; confirm_switch: boolean }
    hidden_providers: string[]
}

export interface Doctor {
    version: string
    data_dir: string
    os: string
    vault: string
    vault_ok: boolean
    vault_error: string | null
    path: string[]
    providers: ProviderStatus[]
}

export interface ServerStatus {
    version: string
    os: string
    arch: string
    vault: string
    dataDir: string
    revision: number
    dev: boolean
}
