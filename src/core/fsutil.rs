//! Atomic, permission-preserving file writes. Every file Switcheroo touches (its own state
//! and every CLI credential slot) goes through `write_atomic`: temp file in the same
//! directory, fsync, rename. Mode defaults to 0600 on unix and is preserved when the target
//! already exists.

use std::fs;
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};

pub fn write_atomic(path: &Path, data: &[u8], default_mode: u32) -> Result<()> {
    let dir = path.parent().context("path has no parent")?;
    fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    let existing_mode = fs::metadata(path).ok().map(|m| m.permissions());
    let tmp = dir.join(format!(
        ".{}.switcheroo-{}.tmp",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("file"),
        std::process::id()
    ));
    let mut f = fs::File::create(&tmp).with_context(|| format!("creating {}", tmp.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = match &existing_mode {
            Some(p) => p.clone(),
            None => fs::Permissions::from_mode(default_mode),
        };
        f.set_permissions(perms)?;
    }
    #[cfg(not(unix))]
    {
        let _ = default_mode;
        let _ = &existing_mode;
    }
    f.write_all(data)?;
    f.sync_all()?;
    drop(f);
    fs::rename(&tmp, path).with_context(|| format!("replacing {}", path.display()))?;
    Ok(())
}

pub fn read_opt(path: &Path) -> Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(b) => Ok(Some(b)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
    }
}

pub fn remove_opt(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e).with_context(|| format!("removing {}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_atomic_preserves_existing_mode_and_content() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.json");
        write_atomic(&p, b"one", 0o600).unwrap();
        assert_eq!(fs::read(&p).unwrap(), b"one");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&p, fs::Permissions::from_mode(0o644)).unwrap();
            write_atomic(&p, b"two", 0o600).unwrap();
            assert_eq!(fs::metadata(&p).unwrap().permissions().mode() & 0o777, 0o644);
        }
        assert_eq!(fs::read(&p).unwrap(), b"two");
        assert!(fs::read_dir(dir.path()).unwrap().count() == 1, "no temp files left behind");
    }
}
