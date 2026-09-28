//! Process-table lookups used for the "CLI is running" warning.

use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

/// Whether a process with this pid exists (best effort).
pub fn pid_alive(pid: u32) -> bool {
    let mut sys = System::new();
    let pid = Pid::from_u32(pid);
    sys.refresh_processes_specifics(ProcessesToUpdate::Some(&[pid]), true, ProcessRefreshKind::nothing());
    sys.process(pid).is_some()
}

/// Linux `/proc/<pid>/stat` comm is `TASK_COMM_LEN - 1` bytes. macOS and Windows report the full
/// executable name, so the short comparison is Linux-only.
pub const LINUX_COMM_LEN: usize = 15;

/// `observed` is a process name from the OS. On Linux a configured name longer than 15 bytes is
/// also compared to that prefix, because that is all the kernel keeps. Other platforms stay exact,
/// or a 15-byte name would match a longer one that is not running.
pub fn process_name_matches(observed: &str, configured: &str, comm_limited: bool) -> bool {
    let observed = observed.strip_suffix(".exe").unwrap_or(observed);
    if observed.eq_ignore_ascii_case(configured) {
        return true;
    }
    comm_limited
        && configured.len() > LINUX_COMM_LEN
        && configured.is_char_boundary(LINUX_COMM_LEN)
        && observed.eq_ignore_ascii_case(&configured[..LINUX_COMM_LEN])
}

pub fn any_running(names: &[&str]) -> bool {
    let mut sys = System::new();
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    let comm_limited = cfg!(target_os = "linux");
    sys.processes().values().any(|p| {
        let name = p.name().to_string_lossy();
        names.iter().any(|n| process_name_matches(&name, n, comm_limited))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_name_matches_on_every_platform() {
        assert!(process_name_matches("claude", "claude", false));
        assert!(process_name_matches("Codex", "codex", false));
        assert!(process_name_matches("codex.exe", "codex", false));
        assert!(!process_name_matches("codex-code-mode", "codex-code-mode-host", false));
    }

    #[test]
    fn linux_comm_matches_only_the_truncated_prefix_of_a_longer_name() {
        assert!(process_name_matches("codex-code-mode", "codex-code-mode-host", true));
        assert!(process_name_matches("CODEX-CODE-MODE", "codex-code-mode-host", true));
        assert!(!process_name_matches("codex-code-mod", "codex-code-mode-host", true));
        assert!(!process_name_matches("codex", "codex-code-mode-host", true));
        assert!(process_name_matches("claude", "claude", true));
        assert!(process_name_matches("codex", "codex", true));
    }
}
