//! The provider contract. Everything above this module (CLI, API, tray, web) is generic over
//! `dyn Provider`; adding a CLI means one module here plus one line in `all()`.

pub mod identity;
pub mod slot_provider;
pub mod slots;
pub mod util;

mod claude_code;
mod codex;
mod expo;
mod fly;
mod gemini_cli;
mod github_cli;
mod netlify;
mod npm;
mod railway;
mod turso;
mod vercel;
mod wrangler;

use std::time::Duration;

use anyhow::{Result, bail};

use crate::core::cx::Cx;
use crate::core::model::{Captured, Identity, Installed, ProviderInfo, SecretBlob, Strategy, TierInfo, Usage, Warning};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    Supported,
    Experimental,
    /// Listed with the reason; no operations are attempted.
    #[allow(dead_code)]
    Unsupported(&'static str),
}

/// Static facts about a provider. Owned copies go over the API as `ProviderInfo`.
#[derive(Clone, Debug)]
pub struct ProviderMeta {
    pub id: &'static str,
    pub name: &'static str,
    /// Brand-derived accent, `#RRGGBB`, chosen to read on both light and dark surfaces. Used
    /// for the provider's identity marks (icon, live dot, meters); never for actions.
    pub color: &'static str,
    pub strategy: Strategy,
    pub tier: Tier,
    /// Binary names to look for on PATH (first found wins).
    pub binaries: &'static [&'static str],
    /// Process names that mean "the CLI is running right now".
    pub process_names: &'static [&'static str],
    /// Env vars that make the CLI ignore its stored login.
    pub env_shadow: &'static [&'static str],
    /// Shown after a switch when the CLI caches credentials in memory.
    pub restart_hint: Option<&'static str>,
    /// What is touched and any caveats; shown in the UI and `switcheroo providers`.
    pub notes: &'static str,
    /// The CLI's own login command, run in a terminal by `switcheroo login`.
    pub login: &'static [&'static str],
}

impl ProviderMeta {
    pub fn info(&self) -> ProviderInfo {
        ProviderInfo {
            id: self.id.to_string(),
            name: self.name.to_string(),
            color: self.color.to_string(),
            strategy: self.strategy,
            tier: match self.tier {
                Tier::Supported => TierInfo::Supported,
                Tier::Experimental => TierInfo::Experimental,
                Tier::Unsupported(r) => TierInfo::Unsupported { reason: r.to_string() },
            },
            binaries: self.binaries.iter().map(|s| s.to_string()).collect(),
            env_shadow: self.env_shadow.iter().map(|s| s.to_string()).collect(),
            restart_hint: self.restart_hint.map(str::to_string),
            notes: self.notes.to_string(),
            login_command: self.login.iter().map(|s| s.to_string()).collect(),
            supports_usage: false,
        }
    }
}

pub trait Provider: Send + Sync {
    fn meta(&self) -> &ProviderMeta;

    /// Is the CLI installed? Default: look for `meta().binaries` on PATH and ask `--version`.
    fn detect(&self, cx: &Cx) -> Option<Installed> {
        detect_binary(cx, self.meta().binaries)
    }

    /// Human descriptions of the files/keychain items this provider touches.
    fn slot_descriptions(&self, _cx: &Cx) -> Vec<String> {
        Vec::new()
    }

    /// Cheap hash of the live slot contents; lets the core skip network `whoami` calls when
    /// nothing changed. `None` = not cacheable.
    fn fingerprint(&self, _cx: &Cx) -> Result<Option<u64>> {
        Ok(None)
    }

    /// Non-fatal findings before a switch (env shadowing, running process, unsupported mode).
    fn preflight(&self, cx: &Cx) -> Vec<Warning> {
        default_preflight(self.meta(), cx)
    }

    /// Who is logged in right now. Offline whenever the CLI stores an identity locally.
    fn live_identity(&self, cx: &Cx) -> Result<Option<Identity>>;

    // ---- SlotSwap ----------------------------------------------------------------------

    /// Read the live slot. `Ok(None)` when nothing is logged in.
    fn capture(&self, _cx: &Cx) -> Result<Option<Captured>> {
        bail!("{} does not support capturing credentials", self.meta().name)
    }

    /// Write a previously captured blob back into the live slot.
    fn activate(&self, _cx: &Cx, _secret: &SecretBlob, _identity: &Identity) -> Result<()> {
        bail!("{} does not support restoring credentials", self.meta().name)
    }

    /// Log out locally (empty the slot).
    fn clear(&self, _cx: &Cx) -> Result<()> {
        bail!("{} does not support clearing credentials", self.meta().name)
    }

    // ---- NativeSwitch ------------------------------------------------------------------

    fn native_list(&self, _cx: &Cx) -> Result<Vec<Identity>> {
        bail!("{} has no native account list", self.meta().name)
    }

    fn native_switch(&self, _cx: &Cx, _id: &str) -> Result<()> {
        bail!("{} has no native account switch", self.meta().name)
    }

    // ---- common -------------------------------------------------------------------------

    /// After a switch: does the CLI agree that `expected` is logged in?
    /// `Err` means "could not check" (the core warns instead of rolling back).
    fn verify(&self, cx: &Cx, expected: &Identity) -> Result<bool> {
        Ok(self.live_identity(cx)?.map(|l| l.same_as(expected)).unwrap_or(false))
    }

    fn login_command(&self, _cx: &Cx) -> Vec<String> {
        self.meta().login.iter().map(|s| s.to_string()).collect()
    }

    // ---- usage -------------------------------------------------------------------------

    fn supports_usage(&self) -> bool {
        false
    }

    /// Current usage/quota for the live login, in whatever shape the service reports. May use
    /// the network. `Ok(None)` when nothing is signed in or the service has no such data.
    fn usage(&self, _cx: &Cx) -> Result<Option<Usage>> {
        Ok(None)
    }

    /// `meta().info()` with the dynamic capability flags filled in.
    fn info(&self) -> ProviderInfo {
        let mut info = self.meta().info();
        info.supports_usage = self.supports_usage();
        info
    }
}

/// All providers, in display order.
pub fn all() -> Vec<Box<dyn Provider>> {
    vec![
        claude_code::provider(),
        codex::provider(),
        github_cli::provider(),
        vercel::provider(),
        wrangler::provider(),
        npm::provider(),
        fly::provider(),
        netlify::provider(),
        gemini_cli::provider(),
        turso::provider(),
        expo::provider(),
        railway::provider(),
    ]
}

pub fn detect_binary(cx: &Cx, binaries: &[&str]) -> Option<Installed> {
    let path = cx.find_binary(binaries)?;
    let version = cx
        .run(&[&path.to_string_lossy(), "--version"], Duration::from_secs(10))
        .ok()
        .filter(|o| o.ok())
        .and_then(|o| util::text::version_from_output(&o.stdout));
    Some(Installed { path, version })
}

pub fn default_preflight(meta: &ProviderMeta, cx: &Cx) -> Vec<Warning> {
    let mut out = Vec::new();
    for var in meta.env_shadow {
        if cx.env(var).is_some() {
            out.push(Warning::warn(
                "env-shadow",
                format!("{} is set in the environment; {} will use it instead of the stored login", var, meta.name),
            ));
        }
    }
    if cx.any_process_running(meta.process_names) {
        out.push(Warning::warn(
            "running",
            format!("{} appears to be running; it may not notice the switch until restarted", meta.name),
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_provider_has_a_unique_id_and_a_hex_color() {
        let all = all();
        let mut ids: Vec<&str> = all.iter().map(|p| p.meta().id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), all.len(), "duplicate provider ids");
        for p in &all {
            let c = p.meta().color;
            assert!(
                c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|ch| ch.is_ascii_hexdigit()),
                "{}: bad color {c}",
                p.meta().id
            );
        }
    }
}
