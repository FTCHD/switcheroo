//! macOS keychain access through `/usr/bin/security`, on purpose: CLIs like Claude Code
//! create their items with the same tool, so `security` is on those items' ACLs and reads never
//! prompt. Items we create are likewise trusted to `security`, which survives our binary being
//! rebuilt (a Security.framework caller would be re-prompted after every code-signature change).
//! Secrets are passed hex-encoded on stdin (`security -i`), never on the command line.

use std::io::Write;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, anyhow};

use super::Vault;
use crate::core::model::SecretBlob;

const SECURITY: &str = "/usr/bin/security";
const ERR_SEC_ITEM_NOT_FOUND: i32 = 44;

/// `security -i` reads one line into a 4096-byte buffer. A longer line is still executed,
/// truncated, and then reported as an error. The password bytes are the truncated part.
pub const SECURITY_STDIN_MAX: usize = 4096;

#[derive(Debug)]
pub enum AddPasswordError {
    /// The stdin line would exceed [`SECURITY_STDIN_MAX`]. `security` was not started.
    TooLarge,
    /// `security` ran and rejected the command. The item may already hold a truncated password.
    Failed,
}

impl std::fmt::Display for AddPasswordError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            AddPasswordError::TooLarge => "keychain item is too large to update through /usr/bin/security",
            AddPasswordError::Failed => "security add-generic-password failed",
        })
    }
}

impl std::error::Error for AddPasswordError {}

/// The `security -i` command that stores `data`, including the trailing newline.
pub fn add_generic_password_line(service: &str, account: &str, label: &str, data: &[u8]) -> String {
    let hex: String = data.iter().map(|b| format!("{b:02x}")).collect();
    format!("add-generic-password -U -a {} -s {} -l {} -X {}\n", quote(account), quote(service), quote(label), hex)
}

pub fn find_generic_password(service: &str, account: &str) -> Result<Option<Vec<u8>>> {
    let out = Command::new(SECURITY)
        .args(["find-generic-password", "-s", service, "-a", account, "-w"])
        .stdin(Stdio::null())
        .output()
        .context("running security find-generic-password")?;
    match out.status.code() {
        Some(0) => {
            let mut bytes = out.stdout;
            if bytes.last() == Some(&b'\n') {
                bytes.pop();
            }
            Ok(Some(bytes))
        }
        Some(ERR_SEC_ITEM_NOT_FOUND) => Ok(None),
        code => Err(anyhow!(
            "security find-generic-password failed (exit {:?}): {}",
            code,
            String::from_utf8_lossy(&out.stderr).trim()
        )),
    }
}

pub fn add_generic_password(service: &str, account: &str, label: &str, data: &[u8]) -> Result<(), AddPasswordError> {
    let line = add_generic_password_line(service, account, label, data);
    if line.len() > SECURITY_STDIN_MAX {
        return Err(AddPasswordError::TooLarge);
    }
    let mut child = Command::new(SECURITY)
        .arg("-i")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| AddPasswordError::Failed)?;
    child
        .stdin
        .take()
        .ok_or(AddPasswordError::Failed)?
        .write_all(line.as_bytes())
        .map_err(|_| AddPasswordError::Failed)?;
    let out = child.wait_with_output().map_err(|_| AddPasswordError::Failed)?;
    let combined = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    // Interactive mode exits 0 even when a command fails; the failure is printed.
    // The text is not returned: it is the tail of the password.
    if !out.status.success() || combined.contains("security:") || combined.contains("error") {
        return Err(AddPasswordError::Failed);
    }
    Ok(())
}

pub fn delete_generic_password(service: &str, account: &str) -> Result<()> {
    let out = Command::new(SECURITY)
        .args(["delete-generic-password", "-s", service, "-a", account])
        .stdin(Stdio::null())
        .output()
        .context("running security delete-generic-password")?;
    match out.status.code() {
        Some(0) | Some(ERR_SEC_ITEM_NOT_FOUND) => Ok(()),
        code => Err(anyhow!(
            "security delete-generic-password failed (exit {:?}): {}",
            code,
            String::from_utf8_lossy(&out.stderr).trim()
        )),
    }
}

/// Quote for `security -i`'s tokenizer (double quotes, backslash escapes).
fn quote(s: &str) -> String {
    let mut q = String::with_capacity(s.len() + 2);
    q.push('"');
    for ch in s.chars() {
        match ch {
            '"' => q.push_str("\\\""),
            '\\' => q.push_str("\\\\"),
            '\n' => q.push_str("\\n"),
            c => q.push(c),
        }
    }
    q.push('"');
    q
}

pub struct MacKeychainVault {
    service: &'static str,
}

impl MacKeychainVault {
    pub fn new(service: &'static str) -> MacKeychainVault {
        MacKeychainVault { service }
    }
}

impl Vault for MacKeychainVault {
    fn name(&self) -> &'static str {
        "macos-keychain"
    }

    fn get(&self, key: &str) -> Result<Option<SecretBlob>> {
        Ok(find_generic_password(self.service, key)?.map(SecretBlob::new))
    }

    fn put(&self, key: &str, label: &str, blob: &SecretBlob) -> Result<()> {
        let label = format!("Switcheroo — {label}");
        add_generic_password(self.service, key, &label, blob.as_bytes())?;
        Ok(())
    }

    fn delete(&self, key: &str) -> Result<()> {
        delete_generic_password(self.service, key)
    }

    fn health(&self) -> Result<String> {
        if !std::path::Path::new(SECURITY).exists() {
            return Err(anyhow!("{SECURITY} not found"));
        }
        Ok("macOS login keychain (via /usr/bin/security)".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting() {
        assert_eq!(quote(r#"a "b" \c"#), r#""a \"b\" \\c""#);
    }

    #[test]
    fn stdin_line_refuses_past_4096_without_including_the_password() {
        let small = add_generic_password_line("Claude Code-credentials", "user", "Claude Code-credentials", &[0u8; 16]);
        assert!(small.len() <= SECURITY_STDIN_MAX);
        assert!(small.ends_with('\n'));

        let big =
            add_generic_password_line("Claude Code-credentials", "user", "Claude Code-credentials", &[0xab; 2500]);
        assert!(big.len() > SECURITY_STDIN_MAX);
        assert!(!exceeds("x".repeat(4096).as_str()));
        assert!(exceeds("x".repeat(4097).as_str()));

        let err = AddPasswordError::TooLarge;
        assert_eq!(err.to_string(), "keychain item is too large to update through /usr/bin/security");
        assert!(!AddPasswordError::Failed.to_string().contains("ab"));
    }

    fn exceeds(line: &str) -> bool {
        line.len() > SECURITY_STDIN_MAX
    }

    /// Real keychain round trip under a throwaway service name. Skipped when the keychain is
    /// unavailable (CI without a login session).
    #[test]
    fn keychain_round_trip_if_available() {
        let service = "switcheroo-test";
        let account = format!("unit:{}", std::process::id());
        if let Err(e) = add_generic_password(service, &account, "Switcheroo test", b"{\"secret\":\"x\\\"y\"}") {
            eprintln!("skipping keychain test: {e}");
            return;
        }
        let got = find_generic_password(service, &account).unwrap();
        assert_eq!(got.as_deref(), Some(&b"{\"secret\":\"x\\\"y\"}"[..]));
        // overwrite (-U)
        add_generic_password(service, &account, "Switcheroo test", b"second").unwrap();
        assert_eq!(find_generic_password(service, &account).unwrap().as_deref(), Some(&b"second"[..]));
        delete_generic_password(service, &account).unwrap();
        assert!(find_generic_password(service, &account).unwrap().is_none());
        delete_generic_password(service, &account).unwrap(); // idempotent
    }
}
