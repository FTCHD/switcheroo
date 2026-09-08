//! Data types shared by every layer. Nothing here knows about a specific provider, the vault
//! backend, or a UI. Secrets only ever travel as `SecretBlob`, which redacts itself in `Debug`.

use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

/// How a provider switches accounts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Strategy {
    /// Switcheroo captures/restores the CLI's live credential slot.
    SlotSwap,
    /// The CLI keeps its own account registry; Switcheroo drives its switch command.
    NativeSwitch,
}

/// Support level, as shown to users.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum TierInfo {
    Supported,
    Experimental,
    Unsupported { reason: String },
}

/// Static description of a provider (owned copy of `ProviderMeta`, for JSON).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    pub strategy: Strategy,
    pub tier: TierInfo,
    pub binaries: Vec<String>,
    pub env_shadow: Vec<String>,
    pub restart_hint: Option<String>,
    pub notes: String,
    pub login_command: Vec<String>,
}

/// Who is logged in. `id` is the stable per-provider key (lower-cased email or login).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra: BTreeMap<String, String>,
}

impl Identity {
    pub fn new(id: impl Into<String>) -> Self {
        let raw = id.into();
        let id = normalize_id(&raw);
        Identity { label: raw, id, email: None, extra: BTreeMap::new() }
    }

    pub fn from_email(email: impl Into<String>) -> Self {
        let email = email.into();
        let mut me = Identity::new(email.clone());
        me.email = Some(email);
        me
    }

    pub fn with_extra(mut self, key: &str, value: impl Into<String>) -> Self {
        let v = value.into();
        if !v.is_empty() {
            self.extra.insert(key.to_string(), v);
        }
        self
    }

    pub fn same_as(&self, other: &Identity) -> bool {
        self.id == other.id
    }
}

/// Emails and logins are compared case-insensitively and without surrounding whitespace.
pub fn normalize_id(raw: &str) -> String {
    raw.trim().to_lowercase()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Installed {
    pub path: PathBuf,
    pub version: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warn,
}

/// Non-fatal finding surfaced by preflight/doctor. Never blocks a switch.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Warning {
    pub severity: Severity,
    pub code: String,
    pub message: String,
}

impl Warning {
    pub fn warn(code: &str, message: impl Into<String>) -> Self {
        Warning { severity: Severity::Warn, code: code.to_string(), message: message.into() }
    }
    pub fn info(code: &str, message: impl Into<String>) -> Self {
        Warning { severity: Severity::Info, code: code.to_string(), message: message.into() }
    }
}

/// A saved account. Never contains the secret; that lives in the vault under `vault_key()`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Account {
    pub provider: String,
    pub id: String,
    pub label: String,
    pub identity: Identity,
    pub saved_at: DateTime<Utc>,
    #[serde(default)]
    pub last_used: Option<DateTime<Utc>>,
    /// `false` for native-switch providers (the CLI holds the credential).
    #[serde(default)]
    pub has_secret: bool,
}

impl Account {
    pub fn vault_key(&self) -> String {
        vault_key(&self.provider, &self.id)
    }
}

pub fn vault_key(provider: &str, account_id: &str) -> String {
    format!("{provider}:{account_id}")
}

/// Opaque provider-defined bytes. Zeroed on drop, redacted in Debug.
pub struct SecretBlob(Zeroizing<Vec<u8>>);

impl SecretBlob {
    pub fn new(bytes: Vec<u8>) -> Self {
        SecretBlob(Zeroizing::new(bytes))
    }
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl Clone for SecretBlob {
    fn clone(&self) -> Self {
        SecretBlob::new(self.0.to_vec())
    }
}

impl fmt::Debug for SecretBlob {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SecretBlob(<{} bytes>)", self.0.len())
    }
}

/// Result of `Provider::capture`. `identity` is `None` when the CLI stores a bare token and
/// nothing on disk says who it belongs to (the caller must supply a label).
#[derive(Debug)]
pub struct Captured {
    pub identity: Option<Identity>,
    pub secret: SecretBlob,
    /// Why `identity` is `None` when a lookup was attempted and failed (e.g. `whoami` offline).
    pub identity_error: Option<String>,
}

/// Everything a UI needs to render one provider.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProviderStatus {
    pub info: ProviderInfo,
    pub installed: Option<Installed>,
    pub live: Option<Identity>,
    /// Id of the saved account that matches `live`, if any.
    pub active_account: Option<String>,
    pub accounts: Vec<Account>,
    pub warnings: Vec<Warning>,
    pub slots: Vec<String>,
    pub live_checked_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SwitchOutcome {
    pub provider: String,
    pub account: Account,
    /// Id of the previously live account that was re-captured before switching.
    pub recaptured: Option<String>,
    pub warnings: Vec<Warning>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_normalizes_id_but_keeps_label() {
        let id = Identity::from_email("  Jane@Example.COM ");
        assert_eq!(id.id, "jane@example.com");
        assert_eq!(id.label, "  Jane@Example.COM ");
        assert_eq!(id.email.as_deref(), Some("  Jane@Example.COM "));
    }

    #[test]
    fn secret_blob_debug_is_redacted() {
        let b = SecretBlob::new(b"sk-very-secret".to_vec());
        assert_eq!(format!("{b:?}"), "SecretBlob(<14 bytes>)");
    }
}
