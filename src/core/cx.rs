//! Execution context handed to providers: the home directory, an environment snapshot, the
//! PATH used to find CLIs, and process helpers. Providers never touch `std::env` directly, so
//! tests can point a provider at a temp home with a fake environment and a different OS layout.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    Mac,
    Linux,
    Windows,
}

impl Os {
    pub fn current() -> Os {
        if cfg!(target_os = "macos") {
            Os::Mac
        } else if cfg!(target_os = "windows") {
            Os::Windows
        } else {
            Os::Linux
        }
    }
}

#[derive(Clone, Debug)]
pub struct CmdOutput {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

impl CmdOutput {
    pub fn ok(&self) -> bool {
        self.status == 0
    }
}

#[derive(Clone, Debug)]
pub struct Cx {
    home: PathBuf,
    env: HashMap<String, String>,
    path: Vec<PathBuf>,
    os: Os,
    commands_allowed: bool,
}

impl Cx {
    /// Snapshot of the real process environment, with well-known tool directories appended to
    /// PATH (GUI launches on macOS get a minimal PATH).
    pub fn from_process() -> Result<Cx> {
        let home = dirs::home_dir().context("no home directory")?;
        let env: HashMap<String, String> = std::env::vars().collect();
        let mut cx = Cx { home, env, path: Vec::new(), os: Os::current(), commands_allowed: true };
        cx.rebuild_path();
        Ok(cx)
    }

    /// A context for tests: empty environment (apart from HOME), commands disabled.
    #[cfg(test)]
    pub fn test(home: &Path, os: Os) -> Cx {
        let mut env = HashMap::new();
        env.insert("HOME".to_string(), home.to_string_lossy().into_owned());
        Cx { home: home.to_path_buf(), env, path: Vec::new(), os, commands_allowed: false }
    }

    #[cfg(test)]
    pub fn set_env(&mut self, key: &str, value: &str) {
        self.env.insert(key.to_string(), value.to_string());
        if key == "PATH" {
            self.rebuild_path();
        }
    }

    pub fn env(&self, key: &str) -> Option<&str> {
        self.env.get(key).map(|s| s.as_str()).filter(|s| !s.is_empty())
    }

    pub fn os(&self) -> Os {
        self.os
    }

    pub fn home(&self) -> &Path {
        &self.home
    }

    /// `~/.config`, `%APPDATA%`, or `~/Library/Application Support`.
    pub fn config_dir(&self) -> PathBuf {
        match self.os {
            Os::Mac => self.home.join("Library").join("Application Support"),
            Os::Windows => {
                self.env("APPDATA").map(PathBuf::from).unwrap_or_else(|| self.home.join("AppData").join("Roaming"))
            }
            Os::Linux => self.env("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|| self.home.join(".config")),
        }
    }

    /// `~/.local/share`, `%APPDATA%`, or `~/Library/Application Support`.
    pub fn data_dir(&self) -> PathBuf {
        match self.os {
            Os::Linux => {
                self.env("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| self.home.join(".local").join("share"))
            }
            _ => self.config_dir(),
        }
    }

    /// `%LOCALAPPDATA%` on Windows, otherwise `data_dir()`.
    pub fn local_data_dir(&self) -> PathBuf {
        match self.os {
            Os::Windows => {
                self.env("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(|| self.home.join("AppData").join("Local"))
            }
            _ => self.data_dir(),
        }
    }

    /// macOS `~/Library/Preferences` (used by env-paths style CLIs), otherwise `config_dir()`.
    pub fn preferences_dir(&self) -> PathBuf {
        match self.os {
            Os::Mac => self.home.join("Library").join("Preferences"),
            _ => self.config_dir(),
        }
    }

    pub fn path_string(&self) -> String {
        let sep = if self.os == Os::Windows { ";" } else { ":" };
        self.path.iter().map(|p| p.to_string_lossy().into_owned()).collect::<Vec<_>>().join(sep)
    }

    fn rebuild_path(&mut self) {
        let sep = if Os::current() == Os::Windows { ';' } else { ':' };
        let mut dirs: Vec<PathBuf> = Vec::new();
        let from_env = self.env.get("PATH").map(|p| p.split(sep).map(PathBuf::from).collect::<Vec<_>>());
        for dir in from_env.unwrap_or_default().into_iter().chain(well_known_bin_dirs(&self.home)) {
            if !dir.as_os_str().is_empty() && !dirs.contains(&dir) {
                dirs.push(dir);
            }
        }
        self.path = dirs;
    }

    /// Ask the login shell for its PATH (macOS GUI launches start with `/usr/bin:/bin`).
    pub fn adopt_login_shell_path(&mut self) {
        if Os::current() == Os::Windows {
            return;
        }
        let shell = self.env("SHELL").unwrap_or("/bin/sh").to_string();
        let out = Command::new(&shell)
            .args(["-ilc", "printf '__SWPATH__%s' \"$PATH\""])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .and_then(|c| wait_with_timeout(c, Duration::from_secs(4)));
        if let Ok(out) = out
            && let Some(idx) = out.stdout.find("__SWPATH__")
        {
            let found = out.stdout[idx + "__SWPATH__".len()..].trim().to_string();
            if !found.is_empty() {
                let mut merged = found;
                if let Some(cur) = self.env.get("PATH") {
                    merged.push(':');
                    merged.push_str(cur);
                }
                self.env.insert("PATH".to_string(), merged);
                self.rebuild_path();
            }
        }
    }

    pub fn find_binary(&self, names: &[&str]) -> Option<PathBuf> {
        let exts: &[&str] = if Os::current() == Os::Windows { &[".exe", ".cmd", ".bat", ""] } else { &[""] };
        for name in names {
            for dir in &self.path {
                for ext in exts {
                    let candidate = dir.join(format!("{name}{ext}"));
                    if candidate.is_file() {
                        return Some(candidate);
                    }
                }
            }
        }
        None
    }

    /// Run a CLI non-interactively, capturing output. `argv[0]` is resolved on our PATH.
    pub fn run(&self, argv: &[&str], timeout: Duration) -> Result<CmdOutput> {
        if !self.commands_allowed {
            return Err(anyhow!("external commands are disabled in this context"));
        }
        let (program, args) = argv.split_first().context("empty argv")?;
        let resolved = self.find_binary(&[program]).unwrap_or_else(|| PathBuf::from(program));
        let child = Command::new(&resolved)
            .args(args)
            .env("PATH", self.path_string())
            .env("NO_COLOR", "1")
            .env("CI", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("running {}", resolved.display()))?;
        Ok(wait_with_timeout(child, timeout)?)
    }

    /// Run a CLI attached to the current terminal (logins). Returns the exit code.
    pub fn run_interactive(&self, argv: &[String]) -> Result<i32> {
        if !self.commands_allowed {
            return Err(anyhow!("external commands are disabled in this context"));
        }
        let (program, args) = argv.split_first().context("empty argv")?;
        let resolved = self.find_binary(&[program.as_str()]).unwrap_or_else(|| PathBuf::from(program));
        let status = Command::new(&resolved)
            .args(args)
            .env("PATH", self.path_string())
            .status()
            .with_context(|| format!("running {}", resolved.display()))?;
        Ok(status.code().unwrap_or(1))
    }

    /// Whether any process with one of these names is running (best effort, never fatal).
    pub fn any_process_running(&self, names: &[&str]) -> bool {
        if !self.commands_allowed || names.is_empty() {
            return false;
        }
        crate::core::proc::any_running(names)
    }
}

fn wait_with_timeout(mut child: std::process::Child, timeout: Duration) -> std::io::Result<CmdOutput> {
    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    let out_thread = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(s) = stdout.as_mut() {
            let _ = s.read_to_end(&mut buf);
        }
        buf
    });
    let err_thread = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(s) = stderr.as_mut() {
            let _ = s.read_to_end(&mut buf);
        }
        buf
    });
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if start.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "command timed out"));
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    let stdout = String::from_utf8_lossy(&out_thread.join().unwrap_or_default()).into_owned();
    let stderr = String::from_utf8_lossy(&err_thread.join().unwrap_or_default()).into_owned();
    Ok(CmdOutput { status: status.code().unwrap_or(-1), stdout, stderr })
}

fn well_known_bin_dirs(home: &Path) -> Vec<PathBuf> {
    let mut v = vec![
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
        home.join(".local").join("bin"),
        home.join(".cargo").join("bin"),
        home.join(".fly").join("bin"),
        home.join(".bun").join("bin"),
        home.join(".volta").join("bin"),
        home.join(".npm-global").join("bin"),
        PathBuf::from("/usr/bin"),
        PathBuf::from("/bin"),
    ];
    // nvm: newest installed node's bin dir
    if let Ok(rd) = std::fs::read_dir(home.join(".nvm").join("versions").join("node")) {
        let mut versions: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
        versions.sort();
        if let Some(latest) = versions.pop() {
            v.push(latest.join("bin"));
        }
    }
    v.retain(|p| p.is_dir());
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_specific_dirs_derive_from_home() {
        let home = Path::new("/tmp/home");
        let mac = Cx::test(home, Os::Mac);
        assert_eq!(mac.config_dir(), home.join("Library/Application Support"));
        assert_eq!(mac.preferences_dir(), home.join("Library/Preferences"));
        let linux = Cx::test(home, Os::Linux);
        assert_eq!(linux.config_dir(), home.join(".config"));
        assert_eq!(linux.data_dir(), home.join(".local/share"));
        let mut win = Cx::test(home, Os::Windows);
        win.set_env("APPDATA", "C:\\Users\\x\\AppData\\Roaming");
        assert_eq!(win.config_dir(), PathBuf::from("C:\\Users\\x\\AppData\\Roaming"));
    }

    #[test]
    fn path_is_deduplicated_in_order() {
        let mut cx = Cx::test(Path::new("/tmp/home"), Os::Linux);
        cx.set_env("PATH", "/usr/bin:/bin:/usr/bin::/bin:/opt/x");
        let joined = cx.path_string();
        assert!(joined.starts_with("/usr/bin:/bin:/opt/x"), "{joined}");
        let entries: Vec<&str> = joined.split(':').collect();
        let unique: std::collections::HashSet<&str> = entries.iter().copied().collect();
        assert_eq!(entries.len(), unique.len(), "duplicates in {joined}");
        assert!(!entries.contains(&""));
    }

    #[test]
    fn empty_env_values_count_as_unset() {
        let mut cx = Cx::test(Path::new("/tmp/home"), Os::Linux);
        cx.set_env("B", "1");
        cx.set_env("EMPTY", "");
        assert_eq!(cx.env("B"), Some("1"));
        assert_eq!(cx.env("EMPTY"), None);
        assert_eq!(cx.env("A"), None);
    }
}
