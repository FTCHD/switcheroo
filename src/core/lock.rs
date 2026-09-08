//! One exclusive lock per data dir around anything that touches a credential slot or
//! `state.json`, so a CLI invocation and the tray never interleave a switch.

use std::fs::{File, OpenOptions, TryLockError};
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};

pub struct LockGuard(File);

impl Drop for LockGuard {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

pub fn acquire(path: &Path, timeout: Duration) -> Result<LockGuard> {
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(path)
        .with_context(|| format!("opening lock {}", path.display()))?;
    let start = Instant::now();
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(LockGuard(file)),
            Err(TryLockError::WouldBlock) => {}
            Err(TryLockError::Error(e)) => return Err(e).context("locking"),
        }
        if start.elapsed() > timeout {
            return Err(anyhow!("another Switcheroo operation is in progress (lock {}); try again", path.display()));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_holder_times_out_until_first_drops() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("lock");
        let g = acquire(&p, Duration::from_millis(100)).unwrap();
        assert!(acquire(&p, Duration::from_millis(100)).is_err());
        drop(g);
        assert!(acquire(&p, Duration::from_millis(100)).is_ok());
    }
}
