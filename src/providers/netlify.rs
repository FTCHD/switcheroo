//! Netlify CLI (`netlify`). Native switch: the CLI keeps every login in its config.json
//! (`users` map, `userId` = active) and switches with `netlify switch --email`. Switcheroo reads
//! only the non-secret user fields and stores nothing.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde_json::Value;

use super::util::text::tilde;
use super::{Provider, ProviderMeta, Tier};
use crate::core::cx::{Cx, Os};
use crate::core::model::{Identity, Strategy};

pub struct Netlify {
    meta: ProviderMeta,
}

pub fn provider() -> Box<dyn Provider> {
    Box::new(Netlify {
        meta: ProviderMeta {
            id: "netlify",
            color: "#00AD9F",
            name: "Netlify",
            strategy: Strategy::NativeSwitch,
            tier: Tier::Supported,
            binaries: &["netlify", "ntl"],
            process_names: &[],
            env_shadow: &["NETLIFY_AUTH_TOKEN"],
            restart_hint: None,
            notes: "Uses Netlify's own multi-user config.json: lists the logged-in users and switches with `netlify switch --email`. No credentials are stored by Switcheroo. Add accounts with `netlify login`.",
            login: &["netlify", "login"],
        },
    })
}

pub fn config_file(cx: &Cx) -> PathBuf {
    match cx.os() {
        Os::Mac => cx.preferences_dir().join("netlify").join("config.json"),
        Os::Linux => cx.config_dir().join("netlify").join("config.json"),
        Os::Windows => cx.config_dir().join("netlify").join("Config").join("config.json"),
    }
}

/// (active user id, users) from config.json; auth tokens are never read.
pub type Users = (Option<String>, Vec<(String, Identity)>);

pub fn parse_config(bytes: &[u8]) -> Result<Users> {
    let doc: Value = serde_json::from_slice(bytes).context("netlify config.json")?;
    let active = doc.get("userId").and_then(Value::as_str).map(str::to_string);
    let mut users = Vec::new();
    if let Some(map) = doc.get("users").and_then(Value::as_object) {
        for (uid, u) in map {
            let email = u.get("email").and_then(Value::as_str).unwrap_or("");
            if email.is_empty() {
                continue;
            }
            let mut ident = Identity::from_email(email);
            if let Some(name) = u.get("name").and_then(Value::as_str) {
                ident = ident.with_extra("name", name);
            }
            ident = ident.with_extra("user_id", uid.as_str());
            users.push((uid.clone(), ident));
        }
    }
    Ok((active, users))
}

impl Netlify {
    fn read(&self, cx: &Cx) -> Result<Users> {
        match crate::core::fsutil::read_opt(&config_file(cx))? {
            Some(b) => parse_config(&b),
            None => Ok((None, Vec::new())),
        }
    }
}

impl Provider for Netlify {
    fn meta(&self) -> &ProviderMeta {
        &self.meta
    }

    fn slot_descriptions(&self, cx: &Cx) -> Vec<String> {
        vec![format!("{} (users list, read-only)", tilde(&config_file(cx), cx.home()))]
    }

    fn fingerprint(&self, cx: &Cx) -> Result<Option<u64>> {
        let data = crate::core::fsutil::read_opt(&config_file(cx))?;
        Ok(Some(super::util::fp::fnv1a(&[data])))
    }

    fn live_identity(&self, cx: &Cx) -> Result<Option<Identity>> {
        let (active, users) = self.read(cx)?;
        let Some(active) = active else { return Ok(None) };
        Ok(users.into_iter().find(|(uid, _)| *uid == active).map(|(_, i)| i))
    }

    fn native_list(&self, cx: &Cx) -> Result<Vec<Identity>> {
        Ok(self.read(cx)?.1.into_iter().map(|(_, i)| i).collect())
    }

    fn native_switch(&self, cx: &Cx, id: &str) -> Result<()> {
        let out = cx.run(&["netlify", "switch", "--email", id], Duration::from_secs(30))?;
        if !out.ok() {
            bail!("netlify switch failed: {}{}", out.stdout.trim(), out.stderr.trim());
        }
        Ok(())
    }

    fn verify(&self, cx: &Cx, expected: &Identity) -> Result<bool> {
        // Offline check against config.json first; then ask the CLI, which may need the network.
        if self.live_identity(cx)?.map(|l| l.same_as(expected)).unwrap_or(false) {
            return Ok(true);
        }
        let out = cx.run(&["netlify", "status", "--json"], Duration::from_secs(30))?;
        if !out.ok() {
            return Ok(false);
        }
        let start = out.stdout.find('{').unwrap_or(0);
        let doc: Value = serde_json::from_str(&out.stdout[start..]).context("netlify status JSON")?;
        Ok(contains_string(&doc, &expected.id))
    }
}

fn contains_string(v: &Value, needle: &str) -> bool {
    match v {
        Value::String(s) => s.eq_ignore_ascii_case(needle),
        Value::Array(a) => a.iter().any(|x| contains_string(x, needle)),
        Value::Object(o) => o.values().any(|x| contains_string(x, needle)),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_users_without_touching_tokens() {
        let cfg = br#"{"telemetryDisabled":true,"userId":"u2","users":{"u1":{"id":"u1","name":"One","email":"one@x.io","auth":{"token":"SECRET1"}},"u2":{"id":"u2","name":"Two","email":"Two@x.io","auth":{"token":"SECRET2"}}}}"#;
        let (active, users) = parse_config(cfg).unwrap();
        assert_eq!(active.as_deref(), Some("u2"));
        assert_eq!(users.len(), 2);
        let two = &users.iter().find(|(u, _)| u == "u2").unwrap().1;
        assert_eq!(two.id, "two@x.io");
        assert_eq!(two.extra["name"], "Two");
        assert!(!format!("{two:?}").contains("SECRET"));

        let dir = tempfile::tempdir().unwrap();
        let cx = Cx::test(dir.path(), Os::Mac);
        let f = config_file(&cx);
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(&f, cfg).unwrap();
        let p = provider();
        assert_eq!(p.live_identity(&cx).unwrap().unwrap().id, "two@x.io");
        assert_eq!(p.native_list(&cx).unwrap().len(), 2);
    }
}
