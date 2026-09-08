//! How a provider learns who is logged in: a JSON field, a JWT claim, or the CLI's own
//! `whoami`. Network resolvers only run when the slot changed (see `Provider::fingerprint`).

use std::time::Duration;

use anyhow::{Context, Result};
use serde_json::Value;

use super::util::jwt;
use crate::core::cx::Cx;
use crate::core::model::Identity;

#[derive(Clone, Copy, Debug)]
pub enum Parse {
    /// JSON pointer into the command's stdout.
    Json(&'static str),
    /// First capture group of a regex over stdout.
    Regex(&'static str),
    /// Whole stdout, trimmed.
    Trim,
}

#[derive(Clone, Copy, Debug)]
pub struct Cmd {
    pub argv: &'static [&'static str],
    pub parse: Parse,
    pub is_email: bool,
    /// `(extra key, JSON pointer)` pairs, for `Parse::Json` only.
    pub extra: &'static [(&'static str, &'static str)],
}

#[derive(Clone, Copy, Debug)]
pub enum IdentityResolver {
    JsonPointer {
        slot: usize,
        pointer: &'static str,
        is_email: bool,
        extra: &'static [(&'static str, &'static str)],
    },
    JwtClaim {
        slot: usize,
        token_pointer: &'static str,
        claim: &'static str,
        extra: &'static [(&'static str, &'static str)],
    },
    Command(Cmd),
    /// The CLI stores a bare token; the user names the account.
    #[allow(dead_code)]
    LabelOnly,
    /// First resolver that yields an identity wins.
    Chain(&'static [IdentityResolver]),
}

impl IdentityResolver {
    pub fn is_offline(&self) -> bool {
        match self {
            IdentityResolver::Command(_) => false,
            IdentityResolver::Chain(items) => items.iter().all(|i| i.is_offline()),
            _ => true,
        }
    }

    pub fn resolve(&self, cx: &Cx, slots: &[Option<Vec<u8>>]) -> Result<Option<Identity>> {
        match self {
            IdentityResolver::JsonPointer { slot, pointer, is_email, extra } => {
                let Some(bytes) = slots.get(*slot).and_then(|s| s.as_ref()) else { return Ok(None) };
                let doc: Value = serde_json::from_slice(bytes).context("slot is not JSON")?;
                Ok(from_value(&doc, pointer, *is_email, extra))
            }
            IdentityResolver::JwtClaim { slot, token_pointer, claim, extra } => {
                let Some(bytes) = slots.get(*slot).and_then(|s| s.as_ref()) else { return Ok(None) };
                let doc: Value = serde_json::from_slice(bytes).context("slot is not JSON")?;
                let Some(token) = doc.pointer(token_pointer).and_then(Value::as_str) else { return Ok(None) };
                let claims = jwt::claims(token)?;
                Ok(from_value(&claims, &format!("/{claim}"), true, extra))
            }
            IdentityResolver::Command(cmd) => run_cmd(cx, cmd),
            IdentityResolver::LabelOnly => Ok(None),
            IdentityResolver::Chain(items) => {
                for item in items.iter() {
                    if let Some(id) = item.resolve(cx, slots)? {
                        return Ok(Some(id));
                    }
                }
                Ok(None)
            }
        }
    }
}

pub fn run_cmd(cx: &Cx, cmd: &Cmd) -> Result<Option<Identity>> {
    let out = cx.run(cmd.argv, Duration::from_secs(30))?;
    if !out.ok() {
        return Ok(None);
    }
    let text = out.stdout.trim();
    match cmd.parse {
        Parse::Json(pointer) => {
            let start = text.find(['{', '[']).unwrap_or(0);
            let doc: Value = serde_json::from_str(&text[start..]).context("command output is not JSON")?;
            Ok(from_value(&doc, pointer, cmd.is_email, cmd.extra))
        }
        Parse::Regex(pattern) => {
            let re = regex::Regex::new(pattern).context("bad identity regex")?;
            let combined = format!("{}\n{}", out.stdout, out.stderr);
            Ok(re.captures(&combined).and_then(|c| c.get(1)).map(|m| make(m.as_str(), cmd.is_email)))
        }
        Parse::Trim => {
            let line = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
            if line.is_empty() || line.contains(' ') {
                return Ok(None);
            }
            Ok(Some(make(line, cmd.is_email)))
        }
    }
}

fn from_value(doc: &Value, pointer: &str, is_email: bool, extra: &[(&str, &str)]) -> Option<Identity> {
    let raw = doc.pointer(pointer).and_then(value_string)?;
    if raw.trim().is_empty() {
        return None;
    }
    let mut id = make(&raw, is_email);
    for (key, ptr) in extra {
        if let Some(v) = doc.pointer(ptr).and_then(value_string) {
            id = id.with_extra(key, v);
        }
    }
    Some(id)
}

fn value_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn make(raw: &str, is_email: bool) -> Identity {
    let raw = raw.trim();
    if is_email || raw.contains('@') { Identity::from_email(raw) } else { Identity::new(raw) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::cx::Os;
    use std::path::Path;

    #[test]
    fn json_pointer_with_extras() {
        let cx = Cx::test(Path::new("/tmp/h"), Os::Linux);
        let r = IdentityResolver::JsonPointer {
            slot: 0,
            pointer: "/oauthAccount/emailAddress",
            is_email: true,
            extra: &[("org", "/oauthAccount/organizationName"), ("missing", "/nope")],
        };
        let data = br#"{"oauthAccount":{"emailAddress":"A@x.io","organizationName":"Org"}}"#.to_vec();
        let id = r.resolve(&cx, &[Some(data)]).unwrap().unwrap();
        assert_eq!(id.id, "a@x.io");
        assert_eq!(id.extra.get("org").map(String::as_str), Some("Org"));
        assert!(!id.extra.contains_key("missing"));
        assert!(r.resolve(&cx, &[None]).unwrap().is_none());
    }

    #[test]
    fn chain_falls_through() {
        let cx = Cx::test(Path::new("/tmp/h"), Os::Linux);
        let r = IdentityResolver::Chain(&[
            IdentityResolver::JsonPointer { slot: 0, pointer: "/a", is_email: false, extra: &[] },
            IdentityResolver::JsonPointer { slot: 0, pointer: "/b", is_email: false, extra: &[] },
        ]);
        let id = r.resolve(&cx, &[Some(br#"{"b":"me"}"#.to_vec())]).unwrap().unwrap();
        assert_eq!(id.id, "me");
        assert!(r.is_offline());
    }
}
