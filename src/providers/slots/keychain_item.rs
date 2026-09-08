//! A macOS keychain generic password owned by another CLI (e.g. Claude Code) is the slot.

use anyhow::Result;

use super::Slot;
use crate::vault::macos_security::{add_generic_password, delete_generic_password, find_generic_password};

pub struct KeychainItemSlot {
    pub service: String,
    pub account: String,
}

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
        add_generic_password(&self.service, &self.account, &self.service, data)
    }
    fn clear(&self) -> Result<()> {
        delete_generic_password(&self.service, &self.account)
    }
    fn describe(&self) -> String {
        format!("keychain item \"{}\" ({})", self.service, self.account)
    }
}
