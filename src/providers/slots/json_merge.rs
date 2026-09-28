//! Pure JSON key merge / pick (same rules as `JsonKeysSlot`) and Claude's Keychain-vs-file
//! choice. No I/O, so callers can test this without spawning `security`.

use anyhow::{Context, Result, bail};
use serde_json::{Map, Value};
use zeroize::Zeroize;

/// Result of asking whether the bare Claude Code Keychain item exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeychainProbe {
    Present,
    Absent,
    /// The lookup failed (locked Keychain, `security` missing, anything else).
    Unavailable,
}

/// Where Claude Code's OAuth blob lives for this switch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CredentialBackend {
    Keychain,
    File,
}

/// `Keychain` for `Present` and `Unavailable` on macOS. `File` when the item is absent, and
/// for every probe on other operating systems. Unavailable stays on the Keychain so a locked
/// store does not fall through to the credentials file.
pub fn credential_backend(os_is_mac: bool, probe: KeychainProbe) -> CredentialBackend {
    match (os_is_mac, probe) {
        (true, KeychainProbe::Present | KeychainProbe::Unavailable) => CredentialBackend::Keychain,
        (true, KeychainProbe::Absent) | (false, _) => CredentialBackend::File,
    }
}

/// Map `find-generic-password` to a probe. Password bytes are wiped and never logged.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn classify_probe(found: Result<Option<Vec<u8>>>) -> KeychainProbe {
    match found {
        Ok(Some(mut bytes)) => {
            bytes.zeroize();
            drop(bytes);
            KeychainProbe::Present
        }
        Ok(None) => KeychainProbe::Absent,
        Err(_) => KeychainProbe::Unavailable,
    }
}

/// Start from `existing` (or `{}`), then insert or remove each name in `keys` from `incoming`.
/// Every other key is copied through unchanged.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn merge_json_keys(existing: Option<&[u8]>, incoming: &[u8], keys: &[&str]) -> Result<Vec<u8>> {
    let mut map = match existing {
        Some(bytes) => parse_object(bytes, "existing credential")?,
        None => Map::new(),
    };
    let incoming = parse_object(incoming, "slot data")?;
    for key in keys {
        match incoming.get(*key) {
            Some(value) => {
                map.insert((*key).to_string(), value.clone());
            }
            None => {
                map.remove(*key);
            }
        }
    }
    Ok(serde_json::to_vec(&map)?)
}

/// JSON object of only `keys` that are present, or `None` when none of them are.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn pick_json_keys(existing: &[u8], keys: &[&str]) -> Result<Option<Vec<u8>>> {
    let map = parse_object(existing, "existing credential")?;
    let mut picked = Map::new();
    for key in keys {
        if let Some(value) = map.get(*key) {
            picked.insert((*key).to_string(), value.clone());
        }
    }
    if picked.is_empty() {
        return Ok(None);
    }
    Ok(Some(serde_json::to_vec(&picked)?))
}

fn parse_object(bytes: &[u8], what: &str) -> Result<Map<String, Value>> {
    match serde_json::from_slice::<Value>(bytes).with_context(|| format!("{what} is not JSON"))? {
        Value::Object(map) => Ok(map),
        _ => bail!("{what} is not a JSON object"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mac_keychain_wins_when_present_even_if_the_file_exists() {
        assert!(matches!(credential_backend(true, KeychainProbe::Present), CredentialBackend::Keychain));
        assert!(matches!(credential_backend(true, KeychainProbe::Unavailable), CredentialBackend::Keychain));
        assert!(matches!(credential_backend(true, KeychainProbe::Absent), CredentialBackend::File));
        assert!(matches!(credential_backend(false, KeychainProbe::Present), CredentialBackend::File));
    }

    #[test]
    fn probe_mapping_drops_the_password() {
        assert!(matches!(classify_probe(Ok(Some(b"secret-password".to_vec()))), KeychainProbe::Present));
        assert!(matches!(classify_probe(Ok(None)), KeychainProbe::Absent));
        assert!(matches!(classify_probe(Err(anyhow::anyhow!("keychain locked"))), KeychainProbe::Unavailable));
    }

    #[test]
    fn merge_replaces_oauth_and_keeps_mcp() {
        let existing = br#"{"claudeAiOauth":{"accessToken":"old"},"mcpOAuth":{"notion":{"accessToken":"keep"}}}"#;
        let incoming = br#"{"claudeAiOauth":{"accessToken":"new"}}"#;
        let out: serde_json::Value =
            serde_json::from_slice(&merge_json_keys(Some(existing), incoming, &["claudeAiOauth"]).unwrap()).unwrap();
        assert_eq!(out["claudeAiOauth"]["accessToken"], "new");
        assert_eq!(out["mcpOAuth"]["notion"]["accessToken"], "keep");
    }

    #[test]
    fn clear_drops_oauth_and_keeps_mcp() {
        let existing = br#"{"claudeAiOauth":{"accessToken":"old"},"mcpOAuth":{"notion":{}}}"#;
        let out: serde_json::Value =
            serde_json::from_slice(&merge_json_keys(Some(existing), b"{}", &["claudeAiOauth"]).unwrap()).unwrap();
        assert!(out.get("claudeAiOauth").is_none());
        assert!(out.get("mcpOAuth").is_some());
    }

    #[test]
    fn pick_returns_only_named_keys_or_none() {
        let existing = br#"{"claudeAiOauth":{"accessToken":"t"},"mcpOAuth":{"notion":{}}}"#;
        let out: serde_json::Value =
            serde_json::from_slice(&pick_json_keys(existing, &["claudeAiOauth"]).unwrap().unwrap()).unwrap();
        assert_eq!(out["claudeAiOauth"]["accessToken"], "t");
        assert!(out.get("mcpOAuth").is_none());
        assert!(pick_json_keys(br#"{"mcpOAuth":{}}"#, &["claudeAiOauth"]).unwrap().is_none());
    }

    #[test]
    fn merge_starts_from_empty_and_ignores_unlisted_incoming_keys() {
        let incoming = br#"{"claudeAiOauth":{"accessToken":"new"},"mcpOAuth":{"evil":true}}"#;
        let out: serde_json::Value =
            serde_json::from_slice(&merge_json_keys(None, incoming, &["claudeAiOauth"]).unwrap()).unwrap();
        assert_eq!(out["claudeAiOauth"]["accessToken"], "new");
        assert!(out.get("mcpOAuth").is_none());
    }
}
