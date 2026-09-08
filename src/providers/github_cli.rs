//! GitHub CLI (`gh`). Native switch: gh keeps every logged-in account itself (hosts.yml plus
//! keychain items), so Switcheroo only drives `gh auth switch` and stores nothing.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde_json::Value;

use super::{Provider, ProviderMeta, Tier};
use crate::core::cx::Cx;
use crate::core::model::{Identity, Strategy};

const HOST: &str = "github.com";

pub struct GithubCli {
    meta: ProviderMeta,
}

pub fn provider() -> Box<dyn Provider> {
    Box::new(GithubCli {
        meta: ProviderMeta {
            id: "github-cli",
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
    fn parses_hosts_json_after_preamble() {
        let s = "You are not logged in...\n{\"hosts\":{\"github.com\":[{\"login\":\"alice\",\"active\":true},{\"login\":\"bob\",\"active\":false}]}}\n";
        let h = parse_hosts(s).unwrap();
        assert_eq!(h, vec![("alice".to_string(), true), ("bob".to_string(), false)]);
        assert!(parse_hosts("{\"hosts\":{}}").unwrap().is_empty());
    }
}
