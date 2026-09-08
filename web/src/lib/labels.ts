// Human names for machine identifiers that reach the UI.

const vaultNames: Record<string, string> = {
    'macos-keychain': 'macOS Keychain',
    'windows-credential-manager': 'Windows Credential Manager',
    'secret-service': 'Secret Service',
    file: 'plain file',
}

export function vaultName(id: string): string {
    return vaultNames[id] ?? id
}

export function strategyName(strategy: 'slot-swap' | 'native-switch'): string {
    return strategy === 'native-switch' ? 'Uses the CLI’s own accounts' : 'Swaps the stored login'
}
