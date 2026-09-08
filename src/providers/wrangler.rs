//! Cloudflare Wrangler (`wrangler`). Live slot: the OAuth login file `default.toml`
//! (`oauth_token`, `refresh_token`, `expiration_time`, `scopes`), whose location moved over the
//! years, so the first existing candidate wins.

use std::path::PathBuf;

use super::identity::{Cmd, IdentityResolver, Parse};
use super::slot_provider::SlotProvider;
use super::slots::{FileSlot, Slot};
use super::util::text::tilde;
use super::{Provider, ProviderMeta, Tier};
use crate::core::cx::{Cx, Os};
use crate::core::model::{Strategy, Warning};

fn candidates(cx: &Cx) -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(h) = cx.env("WRANGLER_HOME") {
        v.push(PathBuf::from(h).join("config").join("default.toml"));
    }
    match cx.os() {
        Os::Mac => v.push(cx.preferences_dir().join(".wrangler").join("config").join("default.toml")),
        Os::Linux => v.push(cx.config_dir().join(".wrangler").join("config").join("default.toml")),
        Os::Windows => v.push(cx.config_dir().join(".wrangler").join("config").join("default.toml")),
    }
    v.push(cx.home().join(".config").join(".wrangler").join("config").join("default.toml"));
    v.push(cx.home().join(".wrangler").join("config").join("default.toml"));
    v
}

pub fn config_file(cx: &Cx) -> PathBuf {
    let c = candidates(cx);
    c.iter().find(|p| p.exists()).cloned().unwrap_or_else(|| c[0].clone())
}

fn slots(cx: &Cx) -> Vec<Box<dyn Slot>> {
    let f = config_file(cx);
    vec![Box::new(FileSlot::new(f.clone(), tilde(&f, cx.home())))]
}

fn preflight(cx: &Cx) -> Vec<Warning> {
    let mut w = Vec::new();
    if config_file(cx).with_extension("enc").exists() {
        w.push(Warning::warn(
            "keyring-mode",
            "Wrangler stores an encrypted login (default.enc, key in the OS keyring); Switcheroo only handles the plain default.toml mode. Re-login with `wrangler login --no-use-keyring`.",
        ));
    }
    w
}

const WHOAMI: Cmd = Cmd {
    argv: &["wrangler", "whoami"],
    parse: Parse::Regex(r"(?i)associated with the email\s+([^\s]+@[A-Za-z0-9.-]*[A-Za-z0-9])"),
    is_email: true,
    extra: &[],
};

pub fn provider() -> Box<dyn Provider> {
    Box::new(SlotProvider {
        meta: ProviderMeta {
            id: "wrangler",
            color: "#F38020",
            name: "Cloudflare Wrangler",
            strategy: Strategy::SlotSwap,
            tier: Tier::Supported,
            binaries: &["wrangler"],
            process_names: &["wrangler", "workerd"],
            env_shadow: &["CLOUDFLARE_API_TOKEN"],
            restart_hint: None,
            notes: "Swaps the OAuth login file default.toml (WRANGLER_HOME, ~/.config/.wrangler, ~/Library/Preferences/.wrangler or ~/.wrangler). The email comes from `wrangler whoami` (network) when a login is saved. Encrypted keyring mode is not supported.",
            login: &["wrangler", "login"],
        },
        slots,
        identity: IdentityResolver::Command(WHOAMI),
        verify: Some(WHOAMI),
        extra_preflight: Some(preflight),
        usage: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_first_existing_candidate() {
        let dir = tempfile::tempdir().unwrap();
        let cx = Cx::test(dir.path(), Os::Mac);
        assert_eq!(config_file(&cx), dir.path().join("Library/Preferences/.wrangler/config/default.toml"));
        let legacy = dir.path().join(".wrangler/config");
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(legacy.join("default.toml"), "oauth_token = \"x\"\n").unwrap();
        assert_eq!(config_file(&cx), legacy.join("default.toml"));
        std::fs::write(legacy.join("default.enc"), "").unwrap();
        assert!(provider().preflight(&cx).iter().any(|w| w.code == "keyring-mode"));
    }

    #[test]
    fn whoami_regex() {
        let re = regex::Regex::new(match WHOAMI.parse {
            Parse::Regex(r) => r,
            _ => unreachable!(),
        })
        .unwrap();
        let out = "👋 You are logged in with an OAuth Token, associated with the email dev@example.com.\n┌──┐";
        assert_eq!(re.captures(out).unwrap().get(1).unwrap().as_str(), "dev@example.com");
    }
}
