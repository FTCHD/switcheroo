//! Windows Credential Manager / Linux Secret Service through keyring-core.

use std::sync::OnceLock;

use anyhow::{Result, anyhow};
use keyring_core::{Entry, Error as KeyringError};

use super::Vault;
use crate::core::model::SecretBlob;

fn init_store() -> Result<()> {
    static INIT: OnceLock<std::result::Result<(), String>> = OnceLock::new();
    let r = INIT.get_or_init(|| {
        #[cfg(target_os = "windows")]
        {
            windows_native_keyring_store::Store::new()
                .map(|store| keyring_core::set_default_store(store))
                .map_err(|e| e.to_string())
        }
        #[cfg(target_os = "linux")]
        {
            dbus_secret_service_keyring_store::Store::new()
                .map(|store| keyring_core::set_default_store(store))
                .map_err(|e| e.to_string())
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        {
            Err::<(), String>("no native credential store on this platform".to_string())
        }
    });
    r.clone().map_err(|e| anyhow!("credential store unavailable: {e}"))
}

pub struct KeyringVault {
    service: &'static str,
}

impl KeyringVault {
    pub fn new(service: &'static str) -> Result<KeyringVault> {
        init_store()?;
        Ok(KeyringVault { service })
    }

    fn entry(&self, key: &str) -> Result<Entry> {
        Entry::new(self.service, key).map_err(|e| anyhow!("keyring entry: {e}"))
    }
}

impl Vault for KeyringVault {
    fn name(&self) -> &'static str {
        if cfg!(target_os = "windows") { "windows-credential-manager" } else { "secret-service" }
    }

    fn get(&self, key: &str) -> Result<Option<SecretBlob>> {
        match self.entry(key)?.get_secret() {
            Ok(bytes) => Ok(Some(SecretBlob::new(bytes))),
            Err(KeyringError::NoEntry) => Ok(None),
            Err(e) => Err(anyhow!("reading credential: {e}")),
        }
    }

    fn put(&self, key: &str, _label: &str, blob: &SecretBlob) -> Result<()> {
        self.entry(key)?.set_secret(blob.as_bytes()).map_err(|e| anyhow!("writing credential: {e}"))
    }

    fn delete(&self, key: &str) -> Result<()> {
        match self.entry(key)?.delete_credential() {
            Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
            Err(e) => Err(anyhow!("deleting credential: {e}")),
        }
    }

    fn health(&self) -> Result<String> {
        init_store()?;
        // A probe write/delete proves the daemon is reachable and unlocked.
        let probe = self.entry("__health__")?;
        probe.set_secret(b"ok").map_err(|e| anyhow!("credential store not writable: {e}"))?;
        let _ = probe.delete_credential();
        Ok(format!("{} (keyring-core)", self.name()))
    }
}
