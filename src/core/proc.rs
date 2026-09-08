//! Process-table lookups used for the "CLI is running" warning.

use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};

pub fn any_running(names: &[&str]) -> bool {
    let mut sys = System::new();
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    sys.processes().values().any(|p| {
        let name = p.name().to_string_lossy();
        let name = name.strip_suffix(".exe").unwrap_or(&name);
        names.iter().any(|n| name.eq_ignore_ascii_case(n))
    })
}
