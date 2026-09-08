// Mirrors src/core/model.rs. Keep in sync by hand; the shapes are small.

export type Strategy = 'slot-swap' | 'native-switch'

export type TierInfo =
    | { kind: 'supported' }
    | { kind: 'experimental' }
    | { kind: 'unsupported'; reason: string }

export interface ProviderInfo {
    id: string
    name: string
    /** Brand accent (#RRGGBB) for the provider's identity marks. */
    color: string
    strategy: Strategy
    tier: TierInfo
    binaries: string[]
    env_shadow: string[]
    restart_hint: string | null
    notes: string
    login_command: string[]
    supports_usage: boolean
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

export type UsageItem = {
    label: string
    detail?: string
} & (
    | { kind: 'percent'; used: number; resets_at?: string }
    | { kind: 'gauge'; used: number; limit: number; unit?: string; resets_at?: string }
    | { kind: 'text'; value: string }
)

export interface Usage {
    items: UsageItem[]
    note?: string
    fetched_at: string
}

export interface Autostart {
    enabled: boolean
    supported: boolean
    location: string
    command: string[]
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
