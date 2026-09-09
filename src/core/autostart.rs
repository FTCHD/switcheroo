//! Start the tray at login. Pure registration, nothing is launched or killed here: a
//! LaunchAgent plist on macOS, the per-user `Run` registry value on Windows, an XDG autostart
//! entry on Linux. Each one runs `switcheroo tray --data-dir <dir>` so a custom data dir sticks.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;

#[cfg_attr(not(any(test, target_os = "macos")), allow(dead_code))]
pub const LABEL: &str = "dev.ftchd.switcheroo";

#[derive(Clone, Debug, Serialize)]
pub struct Autostart {
    pub enabled: bool,
    pub supported: bool,
    /// Where the registration lives (file path or registry value), for the UI.
    pub location: String,
    pub command: Vec<String>,
}

pub fn command(exe: &Path, data_dir: &Path) -> Vec<String> {
    vec![
        exe.to_string_lossy().into_owned(),
        "tray".to_string(),
        "--foreground".to_string(),
        "--data-dir".to_string(),
        data_dir.to_string_lossy().into_owned(),
    ]
}

fn exe() -> Result<PathBuf> {
    std::env::current_exe().context("locating the switcheroo binary")
}

pub fn status(data_dir: &Path) -> Result<Autostart> {
    let exe = exe()?;
    Ok(Autostart {
        enabled: platform::is_enabled()?,
        supported: platform::SUPPORTED,
        location: platform::location(),
        command: command(&exe, data_dir),
    })
}

pub fn enable(data_dir: &Path) -> Result<Autostart> {
    let exe = exe()?;
    platform::enable(&exe, data_dir)?;
    status(data_dir)
}

pub fn disable(data_dir: &Path) -> Result<Autostart> {
    platform::disable()?;
    status(data_dir)
}

#[cfg_attr(not(any(test, target_os = "macos")), allow(dead_code))]
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// launchd property list: run once at login, do not respawn, log to ~/Library/Logs.
#[cfg_attr(not(any(test, target_os = "macos")), allow(dead_code))]
pub fn launch_agent_plist(exe: &Path, data_dir: &Path, log_file: &Path) -> String {
    let args = command(exe, data_dir)
        .iter()
        .map(|a| format!("        <string>{}</string>\n", xml_escape(a)))
        .collect::<String>();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{LABEL}</string>
    <key>ProgramArguments</key>
    <array>
{args}    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <false/>
    <key>ProcessType</key>
    <string>Interactive</string>
    <key>StandardOutPath</key>
    <string>{log}</string>
    <key>StandardErrorPath</key>
    <string>{log}</string>
</dict>
</plist>
"#,
        log = xml_escape(&log_file.to_string_lossy())
    )
}

/// XDG desktop entry. Exec arguments are double-quoted per the spec.
#[cfg_attr(any(target_os = "macos", target_os = "windows"), cfg_attr(not(test), allow(dead_code)))]
pub fn desktop_entry(exe: &Path, data_dir: &Path) -> String {
    let exec = command(exe, data_dir)
        .iter()
        .map(|a| format!("\"{}\"", a.replace('\\', "\\\\").replace('"', "\\\"")))
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "[Desktop Entry]\nType=Application\nName=Switcheroo\nComment=Switch the signed-in account of developer CLIs\nExec={exec}\nTerminal=false\nNoDisplay=true\nX-GNOME-Autostart-enabled=true\n"
    )
}

#[cfg(target_os = "macos")]
mod platform {
    use super::*;
    use crate::core::fsutil::{read_opt, remove_opt, write_atomic};

    pub const SUPPORTED: bool = true;

    fn plist_path() -> Result<PathBuf> {
        Ok(dirs::home_dir().context("no home directory")?.join("Library/LaunchAgents").join(format!("{LABEL}.plist")))
    }

    pub fn location() -> String {
        plist_path().map(|p| p.display().to_string()).unwrap_or_default()
    }

    pub fn is_enabled() -> Result<bool> {
        Ok(read_opt(&plist_path()?)?.is_some())
    }

    pub fn enable(exe: &Path, data_dir: &Path) -> Result<()> {
        let home = dirs::home_dir().context("no home directory")?;
        let log_dir = home.join("Library/Logs/switcheroo");
        std::fs::create_dir_all(&log_dir)?;
        let plist = launch_agent_plist(exe, data_dir, &log_dir.join("tray.log"));
        write_atomic(&plist_path()?, plist.as_bytes(), 0o644)
    }

    pub fn disable() -> Result<()> {
        remove_opt(&plist_path()?)
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use super::*;
    use std::process::Command;

    pub const SUPPORTED: bool = true;
    const KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
    const VALUE: &str = "Switcheroo";

    pub fn location() -> String {
        format!(r"{KEY}\{VALUE}")
    }

    pub fn is_enabled() -> Result<bool> {
        let out = Command::new("reg").args(["query", KEY, "/v", VALUE]).output().context("running reg query")?;
        Ok(out.status.success())
    }

    pub fn enable(exe: &Path, data_dir: &Path) -> Result<()> {
        let cmd = command(exe, data_dir).iter().map(|a| format!("\"{a}\"")).collect::<Vec<_>>().join(" ");
        let out = Command::new("reg")
            .args(["add", KEY, "/v", VALUE, "/t", "REG_SZ", "/d", &cmd, "/f"])
            .output()
            .context("running reg add")?;
        if !out.status.success() {
            anyhow::bail!("reg add failed: {}", String::from_utf8_lossy(&out.stderr).trim());
        }
        Ok(())
    }

    pub fn disable() -> Result<()> {
        let out =
            Command::new("reg").args(["delete", KEY, "/v", VALUE, "/f"]).output().context("running reg delete")?;
        if !out.status.success() && is_enabled()? {
            anyhow::bail!("reg delete failed: {}", String::from_utf8_lossy(&out.stderr).trim());
        }
        Ok(())
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod platform {
    use super::*;
    use crate::core::fsutil::{read_opt, remove_opt, write_atomic};

    pub const SUPPORTED: bool = true;

    fn desktop_path() -> Result<PathBuf> {
        Ok(dirs::config_dir().context("no config directory")?.join("autostart").join("switcheroo.desktop"))
    }

    pub fn location() -> String {
        desktop_path().map(|p| p.display().to_string()).unwrap_or_default()
    }

    pub fn is_enabled() -> Result<bool> {
        Ok(read_opt(&desktop_path()?)?.is_some())
    }

    pub fn enable(exe: &Path, data_dir: &Path) -> Result<()> {
        write_atomic(&desktop_path()?, desktop_entry(exe, data_dir).as_bytes(), 0o644)
    }

    pub fn disable() -> Result<()> {
        remove_opt(&desktop_path()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plist_escapes_and_lists_arguments() {
        let p = launch_agent_plist(
            Path::new("/Apps/A&B/switcheroo"),
            Path::new("/Users/x/Library/Application Support/switcheroo"),
            Path::new("/Users/x/Library/Logs/switcheroo/tray.log"),
        );
        assert!(p.contains("<string>/Apps/A&amp;B/switcheroo</string>"));
        assert!(p.contains("<string>tray</string>"));
        assert!(p.contains("<string>--data-dir</string>"));
        assert!(p.contains("<key>RunAtLoad</key>\n    <true/>"));
        assert!(p.contains(LABEL));
    }

    #[test]
    fn desktop_entry_quotes_exec() {
        let d = desktop_entry(Path::new("/usr/local/bin/switcheroo"), Path::new("/home/x/.config/switcheroo"));
        assert!(d.contains(
            "Exec=\"/usr/local/bin/switcheroo\" \"tray\" \"--foreground\" \"--data-dir\" \"/home/x/.config/switcheroo\""
        ));
        assert!(d.starts_with("[Desktop Entry]\n"));
    }
}
