//! Small text helpers shared by providers.

use std::path::Path;

/// `/Users/x/.codex/auth.json` → `~/.codex/auth.json` for display.
pub fn tilde(path: &Path, home: &Path) -> String {
    match path.strip_prefix(home) {
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}

/// First line of a CLI's `--version` output, without a leading `v`.
pub fn version_from_output(stdout: &str) -> Option<String> {
    let line = stdout.lines().map(str::trim).find(|l| !l.is_empty())?;
    let words: Vec<&str> = line.split_whitespace().collect();
    let v = words
        .iter()
        .find(|w| w.chars().next().is_some_and(|c| c.is_ascii_digit()))
        .or_else(|| words.iter().find(|w| w.starts_with('v') && w.chars().nth(1).is_some_and(|c| c.is_ascii_digit())))
        .copied()
        .unwrap_or(line);
    Some(v.trim_start_matches('v').to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_parsing() {
        assert_eq!(version_from_output("gh version 2.96.0 (2026-07-02)\n").as_deref(), Some("2.96.0"));
        assert_eq!(version_from_output("2.1.263 (Claude Code)").as_deref(), Some("2.1.263"));
        assert_eq!(version_from_output("codex-cli 0.145.0").as_deref(), Some("0.145.0"));
        assert_eq!(version_from_output("v10.2.0").as_deref(), Some("10.2.0"));
    }
}
