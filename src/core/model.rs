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
    pub color: String,
    pub strategy: Strategy,
    pub tier: TierInfo,
    pub binaries: Vec<String>,
    pub env_shadow: Vec<String>,
    pub restart_hint: Option<String>,
    pub notes: String,
    pub login_command: Vec<String>,
    /// Whether `usage` may return something for this provider.
    #[serde(default)]
    pub supports_usage: bool,
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

/// Usage/quota as reported by a provider. Providers compose these primitives however their
/// service measures things; every surface renders them generically.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Usage {
    pub items: Vec<UsageItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub fetched_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UsageItem {
    pub label: String,
    #[serde(flatten)]
    pub kind: UsageKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum UsageKind {
    /// Share of a limit already used, 0–100.
    Percent {
        used: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        resets_at: Option<DateTime<Utc>>,
    },
    /// Absolute used/limit in some unit.
    Gauge {
        used: f64,
        limit: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        unit: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        resets_at: Option<DateTime<Utc>>,
    },
    /// Anything else, already formatted by the provider.
    Text { value: String },
}

impl UsageItem {
    pub fn percent(label: impl Into<String>, used: f64, resets_at: Option<DateTime<Utc>>) -> Self {
        UsageItem { label: label.into(), kind: UsageKind::Percent { used, resets_at }, detail: None }
    }
    pub fn gauge(
        label: impl Into<String>,
        used: f64,
        limit: f64,
        unit: Option<&str>,
        resets_at: Option<DateTime<Utc>>,
    ) -> Self {
        UsageItem {
            label: label.into(),
            kind: UsageKind::Gauge { used, limit, unit: unit.map(str::to_string), resets_at },
            detail: None,
        }
    }
    pub fn text(label: impl Into<String>, value: impl Into<String>) -> Self {
        UsageItem { label: label.into(), kind: UsageKind::Text { value: value.into() }, detail: None }
    }
    /// The value as one short string ("42% used", "123 / 5,000 requests", "Max").
    pub fn value_text(&self) -> String {
        match &self.kind {
            UsageKind::Percent { used, .. } => format!("{}% used", used.round() as i64),
            UsageKind::Gauge { used, limit, unit, .. } => match unit {
                Some(u) => format!("{} / {} {u}", thousands(*used), thousands(*limit)),
                None => format!("{} / {}", thousands(*used), thousands(*limit)),
            },
            UsageKind::Text { value } => value.clone(),
        }
    }

    pub fn resets_at(&self) -> Option<DateTime<Utc>> {
        match &self.kind {
            UsageKind::Percent { resets_at, .. } | UsageKind::Gauge { resets_at, .. } => *resets_at,
            UsageKind::Text { .. } => None,
        }
    }

    /// "Session (5h): 42% used · resets in 2h 10m" for menus and tables.
    pub fn summary(&self) -> String {
        let mut s = format!("{}: {}", self.label, self.value_text());
        if let Some(at) = self.resets_at() {
            s.push_str(&format!(" · resets in {}", humanize_until(at)));
        }
        s
    }
}

pub fn thousands(n: f64) -> String {
    let whole = n.round() as i64;
    let digits = whole.abs().to_string();
    let mut out = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    if whole < 0 { format!("-{out}") } else { out }
}

/// "2h 30m", "45m", "3d 4h", or "now" for past instants.
pub fn humanize_until(at: DateTime<Utc>) -> String {
    let secs = (at - Utc::now()).num_seconds();
    if secs <= 0 {
        return "now".to_string();
    }
    let (d, h, m) = (secs / 86_400, (secs % 86_400) / 3600, (secs % 3600) / 60);
    if d > 0 {
        format!("{d}d {h}h")
    } else if h > 0 {
        format!("{h}h {m}m")
    } else {
        format!("{}m", m.max(1))
    }
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
    fn usage_summaries() {
        assert_eq!(thousands(1234567.0), "1,234,567");
        assert_eq!(thousands(999.0), "999");
        let p = UsageItem::percent("Weekly", 12.4, None);
        assert_eq!(p.summary(), "Weekly: 12% used");
        let g =
            UsageItem::gauge("REST", 123.0, 5000.0, Some("requests"), Some(Utc::now() + chrono::Duration::minutes(95)));
        let summary = g.summary();
        assert!(summary.starts_with("REST: 123 / 5,000 requests · resets in 1h 3"), "{summary}");
        assert_eq!(UsageItem::text("Plan", "Max").summary(), "Plan: Max");
    }

    #[test]
    fn secret_blob_debug_is_redacted() {
        let b = SecretBlob::new(b"sk-very-secret".to_vec());
        assert_eq!(format!("{b:?}"), "SecretBlob(<14 bytes>)");
    }
}
