//! Slot adapters: the few ways CLIs keep a live credential. Providers compose these instead of
//! doing file I/O themselves, so every write is atomic and permission-preserving.

pub mod file;
pub mod json_keys;
#[cfg(target_os = "macos")]
pub mod keychain_item;
pub mod lines;
pub mod locked;

use anyhow::Result;

pub use file::FileSlot;
pub use json_keys::JsonKeysSlot;
#[cfg(target_os = "macos")]
pub use keychain_item::KeychainItemSlot;
pub use lines::LinesSlot;
pub use locked::LockedSlot;

pub trait Slot: Send + Sync {
    /// Current contents, `None` when the slot is empty/absent.
    fn read(&self) -> Result<Option<Vec<u8>>>;
    fn write(&self, data: &[u8]) -> Result<()>;
    fn clear(&self) -> Result<()>;
    /// Shown in `doctor`/UI, e.g. `~/.codex/auth.json`.
    fn describe(&self) -> String;
}
