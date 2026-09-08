//! GitHub CLI (`gh`). Native switch: gh keeps every logged-in account itself (hosts.yml plus
//! keychain items), so Switcheroo only drives `gh auth switch` and stores nothing.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde_json::Value;

use super::{Provider, ProviderMeta, Tier};
use crate::core::cx::Cx;
use chrono::{DateTime, Utc};

use crate::core::model::{Identity, Strategy, Usage, UsageItem};

const HOST: &str = "github.com";

pub struct GithubCli {
    meta: ProviderMeta,
}

pub fn provider() -> Box<dyn Provider> {
    Box::new(GithubCli {
        meta: ProviderMeta {
            id: "github-cli",
            color: "#8250DF",
            name: "GitHub CLI",
            strategy: Strategy::NativeSwitch,
            tier: Tier::Supported,
            binaries: &["gh"],
            process_names: &[],
            env_shadow: &["GH_TOKEN", "GITHUB_TOKEN"],
            restart_hint: None,
            notes: "Uses gh's own account registry: `gh auth status --json hosts` to list, `gh auth switch --user` to switch. No credentials are stored by Switcheroo. github.com only; add accounts with `gh auth login`.",
            login: &["gh", "auth", "login"],
        },
    })
}

/// Parse `gh auth status --json hosts` into (login, active) pairs for github.com.
pub fn parse_hosts(stdout: &str) -> Result<Vec<(String, bool)>> {
    let start = stdout.find('{').context("no JSON in gh output")?;
    let doc: Value = serde_json::from_str(&stdout[start..]).context("gh auth status JSON")?;
    let mut out = Vec::new();
    if let Some(users) = doc.pointer(&format!("/hosts/{HOST}")).and_then(Value::as_array) {
        for u in users {
            if let Some(login) = u.get("login").and_then(Value::as_str) {
                out.push((login.to_string(), u.get("active").and_then(Value::as_bool).unwrap_or(false)));
            }
        }
    }
    Ok(out)
}

impl GithubCli {
    fn hosts(&self, cx: &Cx) -> Result<Vec<(String, bool)>> {
        let out = cx.run(&["gh", "auth", "status", "--json", "hosts"], Duration::from_secs(20))?;
        if out.stdout.trim().is_empty() {
            if out.ok() {
                return Ok(Vec::new());
            }
            bail!("gh auth status failed: {}", out.stderr.trim());
        }
        parse_hosts(&out.stdout)
    }
}

/// `gh api rate_limit` → request budgets per API family.
pub fn parse_gh_rate_limit(v: &Value) -> Usage {
    let mut items = Vec::new();
    for (key, label) in [("core", "REST"), ("graphql", "GraphQL"), ("search", "Search")] {
        let Some(r) = v.pointer(&format!("/resources/{key}")) else { continue };
        let (Some(limit), Some(used)) = (r.get("limit").and_then(Value::as_f64), r.get("used").and_then(Value::as_f64))
        else {
            continue;
        };
        let resets_at = r.get("reset").and_then(Value::as_i64).and_then(|t| DateTime::from_timestamp(t, 0));
        items.push(UsageItem::gauge(label, used, limit, Some("requests"), resets_at));
    }
    Usage { items, note: Some("GitHub API requests this hour for the active account.".into()), fetched_at: Utc::now() }
}

fn identity(login: &str) -> Identity {
    Identity::new(login).with_extra("host", HOST)
}

impl Provider for GithubCli {
    fn meta(&self) -> &ProviderMeta {
        &self.meta
    }

    fn slot_descriptions(&self, _cx: &Cx) -> Vec<String> {
        vec!["gh's own account registry (hosts.yml + keychain)".to_string()]
    }

    fn live_identity(&self, cx: &Cx) -> Result<Option<Identity>> {
        Ok(self.hosts(cx)?.into_iter().find(|(_, active)| *active).map(|(login, _)| identity(&login)))
    }

    fn native_list(&self, cx: &Cx) -> Result<Vec<Identity>> {
        Ok(self.hosts(cx)?.into_iter().map(|(login, _)| identity(&login)).collect())
    }

    fn supports_usage(&self) -> bool {
        true
    }

    fn usage(&self, cx: &Cx) -> Result<Option<Usage>> {
        if self.live_identity(cx)?.is_none() {
            return Ok(None);
        }
        let out = cx.run(&["gh", "api", "rate_limit"], Duration::from_secs(20))?;
        if !out.ok() {
            bail!("gh api rate_limit failed: {}", out.stderr.trim());
        }
        let v: Value = serde_json::from_str(out.stdout.trim()).context("gh api rate_limit JSON")?;
        Ok(Some(parse_gh_rate_limit(&v)))
    }

    fn native_switch(&self, cx: &Cx, id: &str) -> Result<()> {
        let out = cx.run(&["gh", "auth", "switch", "--hostname", HOST, "--user", id], Duration::from_secs(20))?;
        if !out.ok() {
            bail!("gh auth switch failed: {}", out.stderr.trim());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rate_limit() {
        let v: Value = serde_json::from_str(
            r#"{"resources":{"core":{"limit":5000,"used":123,"remaining":4877,"reset":1789000000},
                "graphql":{"limit":5000,"used":0,"remaining":5000,"reset":1789000000}}}"#,
        )
        .unwrap();
        let u = parse_gh_rate_limit(&v);
        assert_eq!(u.items.len(), 2);
        assert_eq!(u.items[0].label, "REST");
        match &u.items[0].kind {
            crate::core::model::UsageKind::Gauge { used, limit, unit, resets_at } => {
                assert_eq!((*used, *limit), (123.0, 5000.0));
                assert_eq!(unit.as_deref(), Some("requests"));
                assert!(resets_at.is_some());
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn parses_hosts_json_after_preamble() {
        let s = "You are not logged in...\n{\"hosts\":{\"github.com\":[{\"login\":\"alice\",\"active\":true},{\"login\":\"bob\",\"active\":false}]}}\n";
        let h = parse_hosts(s).unwrap();
        assert_eq!(h, vec![("alice".to_string(), true), ("bob".to_string(), false)]);
        assert!(parse_hosts("{\"hosts\":{}}").unwrap().is_empty());
    }
}
