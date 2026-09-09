//! Self-update from GitHub Releases. The latest version comes from the redirect of
//! `releases/latest` (no API, no tokens), the asset name from the compile target, the download
//! is verified against the release's `SHA256SUMS`, and the running binary is replaced in place
//! (`self-replace` handles the Windows rename dance). Checks are cached for a day and are
//! always on: there is deliberately no setting to disable them.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::Core;
use super::fsutil::{read_opt, write_atomic};

pub const REPO: &str = "ftchd/switcheroo";
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");
const CHECK_TTL: chrono::Duration = chrono::Duration::hours(24);
const MAX_DOWNLOAD: u64 = 256 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub current: String,
    pub latest: String,
    pub available: bool,
    pub asset: Option<String>,
    pub checked_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Installed {
    pub version: String,
    pub path: PathBuf,
}

/// The release asset built for this binary's platform.
pub fn asset_name() -> Option<&'static str> {
    Some(match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "switcheroo-macos-aarch64",
        ("macos", "x86_64") => "switcheroo-macos-x86_64",
        ("linux", "x86_64") => "switcheroo-linux-x86_64",
        ("linux", "aarch64") => "switcheroo-linux-aarch64",
        ("windows", "x86_64") => "switcheroo-windows-x86_64.exe",
        ("windows", "aarch64") => "switcheroo-windows-aarch64.exe",
        _ => return None,
    })
}

pub fn parse_version(v: &str) -> Option<(u64, u64, u64)> {
    let core = v.trim().trim_start_matches('v').split(['-', '+']).next()?;
    let mut parts = core.split('.').map(|p| p.parse::<u64>().ok());
    Some((parts.next()??, parts.next()??, parts.next()??))
}

pub fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_version(latest), parse_version(current)) {
        (Some(l), Some(c)) => l > c,
        _ => false,
    }
}

/// `/releases/tag/v1.2.3` → `1.2.3`.
pub fn tag_from_location(location: &str) -> Option<String> {
    let tag = location.rsplit("/releases/tag/").next()?;
    let tag = tag.split(['?', '#']).next()?.trim_start_matches('v');
    parse_version(tag).map(|_| tag.to_string())
}

/// The line for `asset` in a `sha256sum`-style listing.
pub fn checksum_for(sums: &str, asset: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let mut it = line.split_whitespace();
        let hash = it.next()?;
        let name = it.next()?.trim_start_matches('*');
        (name == asset && hash.len() == 64).then(|| hash.to_ascii_lowercase())
    })
}

fn agent(follow_redirects: bool) -> ureq::Agent {
    let tls = ureq::tls::TlsConfig::builder().provider(ureq::tls::TlsProvider::NativeTls).build();
    ureq::Agent::config_builder()
        .tls_config(tls)
        .timeout_global(Some(Duration::from_secs(60)))
        .http_status_as_error(false)
        .max_redirects(if follow_redirects { 10 } else { 0 })
        .user_agent(format!("switcheroo/{CURRENT}"))
        .build()
        .into()
}

/// Latest published (non-draft, non-prerelease) version, from the `releases/latest` redirect.
pub fn fetch_latest_version() -> Result<String> {
    let url = format!("https://github.com/{REPO}/releases/latest");
    let resp = agent(false).head(&url).call().with_context(|| format!("HEAD {url}"))?;
    let status = resp.status().as_u16();
    let location = resp.headers().get("location").and_then(|v| v.to_str().ok()).unwrap_or("");
    match status {
        301..=308 => {
            tag_from_location(location).ok_or_else(|| anyhow::anyhow!("unexpected release location: {location}"))
        }
        404 => bail!("no published release yet"),
        s => bail!("GitHub answered HTTP {s} for the latest release"),
    }
}

fn get_bytes(url: &str) -> Result<Vec<u8>> {
    let mut resp = agent(true).get(url).call().with_context(|| format!("GET {url}"))?;
    let status = resp.status().as_u16();
    if !(200..300).contains(&status) {
        bail!("HTTP {status} for {url}");
    }
    resp.body_mut().with_config().limit(MAX_DOWNLOAD).read_to_vec().context("reading download")
}

/// Download `version`, verify it, and replace the running executable with it.
pub fn download_and_install(version: &str) -> Result<Installed> {
    let asset = asset_name().context("no prebuilt Switcheroo for this platform; build from source")?;
    let base = format!("https://github.com/{REPO}/releases/download/v{version}");
    let sums = String::from_utf8(get_bytes(&format!("{base}/SHA256SUMS"))?).context("SHA256SUMS is not text")?;
    let expected = checksum_for(&sums, asset).with_context(|| format!("no checksum for {asset} in SHA256SUMS"))?;
    let bytes = get_bytes(&format!("{base}/{asset}"))?;
    let actual: String = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
    if actual != expected {
        bail!("checksum mismatch for {asset}: expected {expected}, got {actual}");
    }

    let exe = std::env::current_exe().context("locating the running binary")?;
    let dir = exe.parent().context("binary has no parent directory")?;
    let tmp = dir.join(format!(".switcheroo-update-{}", std::process::id()));
    if let Err(e) = std::fs::write(&tmp, &bytes) {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            bail!(
                "cannot write next to {} (permission denied). Re-run the installer instead:\n  curl -fsSL https://raw.githubusercontent.com/{REPO}/main/install.sh | sh",
                exe.display()
            );
        }
        return Err(e).with_context(|| format!("writing {}", tmp.display()));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))?;
    }
    let replaced = self_replace::self_replace(&tmp).with_context(|| format!("replacing {}", exe.display()));
    let _ = std::fs::remove_file(&tmp);
    replaced?;
    Ok(Installed { version: version.to_string(), path: exe })
}

/// Start the new binary with this process's arguments and never return.
pub fn relaunch() -> ! {
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("switcheroo"));
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = std::process::Command::new(&exe).args(&args).exec();
        eprintln!("relaunch failed: {err}");
        std::process::exit(1);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        let _ = std::process::Command::new(&exe)
            .args(&args)
            .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
            .spawn();
        std::process::exit(0);
    }
}

impl Core {
    /// Latest-version check, cached for a day in memory and in `update-check.json`. A failed
    /// check returns the last cached answer when there is one.
    pub fn update_status(&self, force: bool) -> Result<UpdateInfo> {
        if !force
            && let Some(cached) = self.update_cache.lock().unwrap().clone()
            && Utc::now() - cached.checked_at < CHECK_TTL
            && cached.current == CURRENT
        {
            return Ok(cached);
        }
        let file = self.dirs.update_file();
        let on_disk: Option<UpdateInfo> = read_opt(&file).ok().flatten().and_then(|b| serde_json::from_slice(&b).ok());
        if !force
            && let Some(cached) = &on_disk
            && Utc::now() - cached.checked_at < CHECK_TTL
            && cached.current == CURRENT
        {
            *self.update_cache.lock().unwrap() = Some(cached.clone());
            return Ok(cached.clone());
        }
        match fetch_latest_version() {
            Ok(latest) => {
                let info = UpdateInfo {
                    current: CURRENT.to_string(),
                    available: is_newer(&latest, CURRENT),
                    latest,
                    asset: asset_name().map(str::to_string),
                    checked_at: Utc::now(),
                };
                let _ = write_atomic(&file, &serde_json::to_vec_pretty(&info)?, 0o600);
                *self.update_cache.lock().unwrap() = Some(info.clone());
                Ok(info)
            }
            Err(e) => match on_disk.filter(|c| c.current == CURRENT) {
                Some(stale) => Ok(stale),
                None => Err(e),
            },
        }
    }

    /// Install the latest release over this binary. The caller decides how to relaunch.
    pub fn install_update(&self) -> Result<Installed> {
        let info = self.update_status(true)?;
        if !info.available {
            bail!("already up to date (v{CURRENT})");
        }
        let installed = download_and_install(&info.latest)?;
        let done = UpdateInfo { current: installed.version.clone(), available: false, ..info };
        let _ = write_atomic(&self.dirs.update_file(), &serde_json::to_vec_pretty(&done)?, 0o600);
        *self.update_cache.lock().unwrap() = Some(done);
        Ok(installed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert_eq!(parse_version("v1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_version("0.10.0-beta.1"), Some((0, 10, 0)));
        assert_eq!(parse_version("nope"), None);
        assert!(is_newer("0.2.0", "0.1.9"));
        assert!(is_newer("1.0.0", "0.99.99"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("garbage", "0.1.0"));
    }

    #[test]
    fn release_tag_from_redirect() {
        assert_eq!(
            tag_from_location("https://github.com/ftchd/switcheroo/releases/tag/v0.2.0").as_deref(),
            Some("0.2.0")
        );
        assert_eq!(tag_from_location("/ftchd/switcheroo/releases/tag/v1.0.0?x=1").as_deref(), Some("1.0.0"));
        assert_eq!(tag_from_location("https://github.com/ftchd/switcheroo/releases"), None);
    }

    #[test]
    fn checksum_lookup() {
        let sums = "aaaa  switcheroo-linux-x86_64\n\
                    0123456789abcdef0123456789abcdef0123456789abcdef0123456789ABCDEF *switcheroo-macos-aarch64\n";
        assert_eq!(
            checksum_for(sums, "switcheroo-macos-aarch64").as_deref(),
            Some("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")
        );
        assert_eq!(checksum_for(sums, "switcheroo-linux-x86_64"), None, "short hash rejected");
        assert_eq!(checksum_for(sums, "other"), None);
    }

    #[test]
    fn this_platform_has_an_asset() {
        assert!(asset_name().is_some());
    }
}
