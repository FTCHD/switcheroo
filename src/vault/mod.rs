//! Where saved credentials live. Keys are `<provider>:<account-id>`; values are opaque
//! provider blobs. The backend is picked once at startup: the OS credential store by default,
//! a 0600 JSON file only when explicitly chosen.

pub mod file_store;
#[cfg(not(target_os = "macos"))]
pub mod keyring_store;
#[cfg(target_os = "macos")]
pub mod macos_security;

use anyhow::{Context, Result, anyhow};

use crate::core::appdirs::AppDirs;
use crate::core::model::SecretBlob;
use crate::core::settings::VaultChoice;

pub const SERVICE: &str = "switcheroo";

pub trait Vault: Send + Sync {
    fn name(&self) -> &'static str;
    fn get(&self, key: &str) -> Result<Option<SecretBlob>>;
    fn put(&self, key: &str, label: &str, blob: &SecretBlob) -> Result<()>;
    fn delete(&self, key: &str) -> Result<()>;
    /// Human description of the backend and whether it is reachable.
    fn health(&self) -> Result<String>;
}

pub fn open(choice: VaultChoice, dirs: &AppDirs) -> Result<Box<dyn Vault>> {
    match choice {
        VaultChoice::File => Ok(Box::new(file_store::FileVault::new(dirs.vault_file()))),
        VaultChoice::Keychain | VaultChoice::Auto => {
            let vault = native().context("opening the OS credential store")?;
            vault.health().map_err(|e| {
                anyhow!(
                    "{e}. Use `--vault file` (or set \"vault\": \"file\" in settings.json) to store \
                     secrets in a 0600 file instead."
                )
            })?;
            Ok(vault)
        }
    }
}

#[cfg(target_os = "macos")]
fn native() -> Result<Box<dyn Vault>> {
    Ok(Box::new(macos_security::MacKeychainVault::new(SERVICE)))
}

#[cfg(not(target_os = "macos"))]
fn native() -> Result<Box<dyn Vault>> {
    Ok(Box::new(keyring_store::KeyringVault::new(SERVICE)?))
}
