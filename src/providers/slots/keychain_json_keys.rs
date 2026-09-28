//! A macOS keychain generic password whose value is a JSON object. Only the named keys are
//! the slot; every other key in the item is written back unchanged.

use anyhow::{Context, Result, bail};
use serde_json::Value;
use zeroize::Zeroizing;

use super::Slot;
use super::json_merge::{merge_json_keys, pick_json_keys};
use crate::vault::macos_security::{
    AddPasswordError, add_generic_password, delete_generic_password, find_generic_password,
};

pub struct KeychainJsonKeysSlot {
    pub service: String,
    pub account: String,
    pub keys: Vec<&'static str>,
}

impl KeychainJsonKeysSlot {
    pub fn new(service: &str, account: &str, keys: &[&'static str]) -> Self {
        Self { service: service.to_string(), account: account.to_string(), keys: keys.to_vec() }
    }

    fn load(&self) -> Result<Option<Zeroizing<Vec<u8>>>> {
        Ok(find_generic_password(&self.service, &self.account)?.map(Zeroizing::new))
    }

    /// `label` is the service name, same as `KeychainItemSlot`. An empty object deletes the item.
    /// `preimage` is the password before this update. A failed `security` invocation may already
    /// have stored a truncated password, so the previous bytes are written back.
    fn store(&self, merged: &[u8], preimage: Option<&[u8]>) -> Result<()> {
        let value: Value = serde_json::from_slice(merged).context("merged credential is not JSON")?;
        match value.as_object() {
            Some(map) if map.is_empty() => delete_generic_password(&self.service, &self.account),
            Some(_) => apply_password(merged, preimage, |bytes| {
                add_generic_password(&self.service, &self.account, &self.service, bytes)
            }),
            None => bail!("merged credential is not a JSON object"),
        }
    }
}

/// Send `merged`. If `security` was not started, leave the item alone. If it was started and
/// failed, write `preimage` back. Errors never include the password.
fn apply_password(
    merged: &[u8],
    preimage: Option<&[u8]>,
    mut write: impl FnMut(&[u8]) -> Result<(), AddPasswordError>,
) -> Result<()> {
    match write(merged) {
        Ok(()) => Ok(()),
        Err(AddPasswordError::TooLarge) => {
            bail!("keychain item is too large to update through /usr/bin/security")
        }
        Err(AddPasswordError::Failed) => {
            if let Some(prev) = preimage {
                match write(prev) {
                    Ok(()) => bail!("keychain update failed; restored the previous item"),
                    Err(_) => bail!("keychain update failed, and the previous item could not be restored"),
                }
            }
            bail!("security add-generic-password failed")
        }
    }
}

impl Slot for KeychainJsonKeysSlot {
    fn read(&self) -> Result<Option<Vec<u8>>> {
        let Some(bytes) = self.load()? else { return Ok(None) };
        pick_json_keys(&bytes, &self.keys)
    }

    fn write(&self, data: &[u8]) -> Result<()> {
        let existing = self.load()?;
        let merged = Zeroizing::new(merge_json_keys(existing.as_deref().map(Vec::as_slice), data, &self.keys)?);
        self.store(&merged, existing.as_deref().map(Vec::as_slice))
    }

    fn clear(&self) -> Result<()> {
        let existing = self.load()?;
        if existing.is_none() {
            return Ok(());
        }
        let merged = Zeroizing::new(merge_json_keys(existing.as_deref().map(Vec::as_slice), b"{}", &self.keys)?);
        self.store(&merged, existing.as_deref().map(Vec::as_slice))
    }

    fn describe(&self) -> String {
        format!("keychain item \"{}\" ({}) → {}", self.service, self.account, self.keys.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oversized_update_is_not_sent_and_does_not_restore() {
        let mut calls = Vec::new();
        let err = apply_password(br#"{"claudeAiOauth":{"accessToken":"new"}}"#, Some(br#"{"mcpOAuth":{}}"#), |bytes| {
            calls.push(bytes.to_vec());
            Err(AddPasswordError::TooLarge)
        })
        .unwrap_err();
        assert_eq!(calls.len(), 1);
        let msg = err.to_string();
        assert!(msg.contains("too large"));
        assert!(!msg.contains("mcpOAuth"));
        assert!(!msg.contains("accessToken"));
    }

    #[test]
    fn failed_update_writes_the_preimage_back() {
        let mut calls = Vec::new();
        let err = apply_password(
            br#"{"claudeAiOauth":{"accessToken":"new"}}"#,
            Some(br#"{"mcpOAuth":{"notion":{}}}"#),
            |bytes| {
                calls.push(bytes.to_vec());
                if calls.len() == 1 { Err(AddPasswordError::Failed) } else { Ok(()) }
            },
        )
        .unwrap_err();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[1], br#"{"mcpOAuth":{"notion":{}}}"#);
        let msg = err.to_string();
        assert!(msg.contains("restored the previous item"));
        assert!(!msg.contains("notion"));
        assert!(!msg.contains("accessToken"));
    }

    #[test]
    fn failed_update_without_a_preimage_does_not_invent_one() {
        let mut calls = 0;
        let err = apply_password(br#"{"claudeAiOauth":{}}"#, None, |_| {
            calls += 1;
            Err(AddPasswordError::Failed)
        })
        .unwrap_err();
        assert_eq!(calls, 1);
        assert_eq!(err.to_string(), "security add-generic-password failed");
    }
}
