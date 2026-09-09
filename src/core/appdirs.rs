//! Where Switcheroo keeps its own files. Everything lives under one directory so a custom
//! `--data-dir` / `SWITCHEROO_DATA_DIR` relocates the whole installation (tests use a temp dir).

use std::path::PathBuf;

use anyhow::{Context, Result};

pub const APP_NAME: &str = "switcheroo";

#[derive(Clone, Debug)]
pub struct AppDirs {
    pub data: PathBuf,
}

impl AppDirs {
    pub fn resolve(override_dir: Option<PathBuf>) -> Result<AppDirs> {
        let data = match override_dir {
            Some(d) => d,
            None => match std::env::var_os("SWITCHEROO_DATA_DIR") {
                Some(d) if !d.is_empty() => PathBuf::from(d),
                _ => dirs::config_dir().context("could not determine the user config directory")?.join(APP_NAME),
            },
        };
        std::fs::create_dir_all(&data).with_context(|| format!("creating data dir {}", data.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&data, std::fs::Permissions::from_mode(0o700));
        }
        Ok(AppDirs { data })
    }

    pub fn state_file(&self) -> PathBuf {
        self.data.join("state.json")
    }
    pub fn settings_file(&self) -> PathBuf {
        self.data.join("settings.json")
    }
    pub fn lock_file(&self) -> PathBuf {
        self.data.join("lock")
    }
    pub fn vault_file(&self) -> PathBuf {
        self.data.join("vault.json")
    }
    pub fn server_file(&self) -> PathBuf {
        self.data.join("server.json")
    }
    pub fn update_file(&self) -> PathBuf {
        self.data.join("update-check.json")
    }
}
