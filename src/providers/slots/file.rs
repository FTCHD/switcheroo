//! A whole file is the credential slot.

use std::path::PathBuf;

use anyhow::Result;

use super::Slot;
use crate::core::fsutil::{read_opt, remove_opt, write_atomic};

pub struct FileSlot {
    pub path: PathBuf,
    pub mode: u32,
    pub display: String,
}

impl FileSlot {
    pub fn new(path: PathBuf, display: String) -> FileSlot {
        FileSlot { path, mode: 0o600, display }
    }
}

impl Slot for FileSlot {
    fn read(&self) -> Result<Option<Vec<u8>>> {
        read_opt(&self.path)
    }
    fn write(&self, data: &[u8]) -> Result<()> {
        write_atomic(&self.path, data, self.mode)
    }
    fn clear(&self) -> Result<()> {
        remove_opt(&self.path)
    }
    fn describe(&self) -> String {
        self.display.clone()
    }
}
