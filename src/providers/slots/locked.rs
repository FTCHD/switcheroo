//! Wraps a slot so writes first take the CLI's own lock file (flyctl keeps one).

use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;

use super::Slot;

pub struct LockedSlot {
    pub inner: Box<dyn Slot>,
    pub lock_path: PathBuf,
}

impl LockedSlot {
    pub fn new(inner: Box<dyn Slot>, lock_path: PathBuf) -> LockedSlot {
        LockedSlot { inner, lock_path }
    }
}

impl Slot for LockedSlot {
    fn read(&self) -> Result<Option<Vec<u8>>> {
        self.inner.read()
    }
    fn write(&self, data: &[u8]) -> Result<()> {
        let _g = crate::core::lock::acquire(&self.lock_path, Duration::from_secs(5))?;
        self.inner.write(data)
    }
    fn clear(&self) -> Result<()> {
        let _g = crate::core::lock::acquire(&self.lock_path, Duration::from_secs(5))?;
        self.inner.clear()
    }
    fn describe(&self) -> String {
        self.inner.describe()
    }
}
