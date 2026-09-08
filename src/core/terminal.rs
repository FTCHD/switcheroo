//! Open the user's terminal running a command (logins need a TTY and a browser). Best effort:
//! returns Ok(false) when no terminal could be launched so callers can show the command.

use std::process::{Command, Stdio};

use anyhow::Result;

pub fn open_terminal(command_line: &str) -> Result<bool> {
    if cfg!(target_os = "macos") {
        let script = format!(
            "tell application \"Terminal\"\n activate\n do script \"{}\"\nend tell",
            command_line.replace('\\', "\\\\").replace('"', "\\\"")
        );
        let status = Command::new("osascript")
            .args(["-e", &script])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;
        return Ok(status.success());
    }
    if cfg!(target_os = "windows") {
        let status = Command::new("cmd")
            .args(["/c", "start", "\"Switcheroo\"", "cmd", "/k", command_line])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;
        return Ok(status.success());
    }
    let shell_cmd = format!("{command_line}; echo; echo '[switcheroo] done — press Enter to close'; read _");
    let mut candidates: Vec<Vec<String>> = Vec::new();
    if let Ok(term) = std::env::var("TERMINAL")
        && !term.is_empty()
    {
        candidates.push(vec![term, "-e".into(), "sh".into(), "-c".into(), shell_cmd.clone()]);
    }
    for (bin, args) in [
        ("x-terminal-emulator", vec!["-e", "sh", "-c"]),
        ("gnome-terminal", vec!["--", "sh", "-c"]),
        ("konsole", vec!["-e", "sh", "-c"]),
        ("xfce4-terminal", vec!["-e", "sh -c"]),
        ("alacritty", vec!["-e", "sh", "-c"]),
        ("kitty", vec!["sh", "-c"]),
        ("xterm", vec!["-e", "sh", "-c"]),
    ] {
        let mut v: Vec<String> = vec![bin.to_string()];
        v.extend(args.iter().map(|s| s.to_string()));
        v.push(shell_cmd.clone());
        candidates.push(v);
    }
    for argv in candidates {
        let (bin, args) = argv.split_first().unwrap();
        let spawned =
            Command::new(bin).args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
        if spawned.is_ok() {
            return Ok(true);
        }
    }
    Ok(false)
}
