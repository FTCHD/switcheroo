//! A macOS keychain generic password, taken whole. Claude Code does not use this: its item
//! also holds sibling keys such as `mcpOAuth`, and those go through `KeychainJsonKeysSlot`.

use anyhow::Result;

use super::Slot;
use crate::vault::macos_security::{add_generic_password, delete_generic_password, find_generic_password};

#[allow(dead_code)]
pub struct KeychainItemSlot {
    pub service: String,
    pub account: String,
}

#[allow(dead_code)]
impl KeychainItemSlot {
    pub fn new(service: &str, account: &str) -> KeychainItemSlot {
        KeychainItemSlot { service: service.to_string(), account: account.to_string() }
    }
}

impl Slot for KeychainItemSlot {
    fn read(&self) -> Result<Option<Vec<u8>>> {
        find_generic_password(&self.service, &self.account)
    }
    fn write(&self, data: &[u8]) -> Result<()> {
        add_generic_password(&self.service, &self.account, &self.service, data)?;
        Ok(())
    }
    fn clear(&self) -> Result<()> {
        delete_generic_password(&self.service, &self.account)
    }
    fn describe(&self) -> String {
        format!("keychain item \"{}\" ({})", self.service, self.account)
    }
}
