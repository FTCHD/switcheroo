//! `settings.json`: user preferences. Missing keys take defaults so old files keep working.

use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::fsutil::{read_opt, write_atomic};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum VaultChoice {
    /// OS credential store, falling back to nothing (errors) when unavailable.
    #[default]
    Auto,
    Keychain,
    /// Plaintext JSON (0600) under the data dir. Opt-in.
    File,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct TraySettings {
    pub show_emails: bool,
    pub confirm_switch: bool,
}

impl Default for TraySettings {
    fn default() -> Self {
        TraySettings { show_emails: true, confirm_switch: false }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub vault: VaultChoice,
    /// `host:port` for the embedded server; `None` = deterministic loopback port.
    pub bind: Option<String>,
    pub tray: TraySettings,
    pub hidden_providers: Vec<String>,
}

impl Settings {
    pub fn load(path: &Path) -> Result<Settings> {
        match read_opt(path)? {
            Some(bytes) => serde_json::from_slice(&bytes).with_context(|| format!("parsing {}", path.display())),
            None => Ok(Settings::default()),
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        write_atomic(path, &serde_json::to_vec_pretty(self)?, 0o600)
    }
}
