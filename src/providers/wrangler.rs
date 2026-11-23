//! Cloudflare Wrangler (`wrangler`). Live slot: the OAuth login file of the default profile,
//! `config/default.toml` (`oauth_token`, `refresh_token`, `expiration_time`, `scopes`). The
//! folder is found the way Wrangler 4 finds it.

use std::path::PathBuf;

use serde_json::Value;

use super::identity::{Cmd, IdentityResolver, Parse};
use super::slot_provider::SlotProvider;
use super::slots::{FileSlot, Slot};
use super::util::text::tilde;
use super::{Provider, ProviderMeta, Tier};
use crate::core::cx::{Cx, Os};
use crate::core::model::{Strategy, Warning};

/// `~/.wrangler` while that legacy folder exists, otherwise `.wrangler` in the XDG config folder
/// (`XDG_CONFIG_HOME`, or `~/Library/Preferences`, `~/.config`, `%APPDATA%\xdg.config`).
fn config_root(cx: &Cx) -> PathBuf {
    let legacy = cx.home().join(".wrangler");
    if legacy.is_dir() {
        return legacy;
    }
    let xdg = match cx.env("XDG_CONFIG_HOME") {
        Some(dir) => PathBuf::from(dir),
        None => match cx.os() {
            Os::Mac => cx.preferences_dir(),
            Os::Linux => cx.home().join(".config"),
            Os::Windows => cx.config_dir().join("xdg.config"),
        },
    };
    xdg.join(".wrangler")
}

/// The default profile is `default`, or `staging` when `WRANGLER_API_ENVIRONMENT=staging`.
fn default_profile(cx: &Cx) -> &'static str {
    if cx.env("WRANGLER_API_ENVIRONMENT") == Some("staging") { "staging" } else { "default" }
}

fn config_file(cx: &Cx) -> PathBuf {
    config_root(cx).join("config").join(format!("{}.toml", default_profile(cx)))
}

fn slots(cx: &Cx) -> Vec<Box<dyn Slot>> {
    let f = config_file(cx);
    vec![Box::new(FileSlot::new(f.clone(), tilde(&f, cx.home())))]
}

/// Named profiles (`wrangler auth profile`): their login files next to the default one, and
/// folders bound to a profile in `profiles/directory-bindings.json`.
fn profiles(cx: &Cx) -> (Vec<String>, usize) {
    let root = config_root(cx);
    let mut names: Vec<String> = std::fs::read_dir(root.join("config"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            let ext = path.extension()?.to_str()?;
            let stem = path.file_stem()?.to_str()?;
            (matches!(ext, "toml" | "enc") && !matches!(stem, "default" | "staging")).then(|| stem.to_string())
        })
        .collect();
    let bindings: Option<Value> = std::fs::read(root.join("profiles").join("directory-bindings.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok());
    let bound = bindings.as_ref().and_then(Value::as_object).map(|m| {
        for profile in m.values().filter_map(Value::as_str) {
            names.push(profile.to_string());
        }
        m.len()
    });
    names.sort();
    names.dedup();
    (names, bound.unwrap_or(0))
}

fn preflight(cx: &Cx) -> Vec<Warning> {
    let mut w = Vec::new();
    let profile = default_profile(cx);
    if config_file(cx).with_extension("enc").exists() {
        w.push(Warning::warn(
            "keyring-mode",
            format!(
                "Wrangler stores an encrypted login ({profile}.enc, key in the OS keyring); Switcheroo only handles the plain {profile}.toml mode. Sign in again with `wrangler login --no-use-keyring`."
            ),
        ));
    }
    let (names, bound) = profiles(cx);
    if !names.is_empty() {
        let folders = match bound {
            0 => String::new(),
            1 => " and 1 folder bound to a profile".to_string(),
            n => format!(" and {n} folders bound to profiles"),
        };
        w.push(Warning::warn(
            "profiles",
            format!(
                "Wrangler has profiles ({}){folders}. Switcheroo switches only the default login; in a folder bound to a profile, Wrangler keeps using that profile's login.",
                names.join(", ")
            ),
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
            notes: "Swaps the OAuth login file of the default profile, config/default.toml in ~/.wrangler or the .wrangler config folder (~/Library/Preferences, ~/.config, XDG_CONFIG_HOME). Folders bound to a Wrangler profile keep that profile's login. The email comes from `wrangler whoami` (network) when a login is saved. Wrangler replaces its refresh token on every refresh, so Switcheroo saves the live login again whenever it changes. If `wrangler login` replaces a login before Switcheroo sees its latest refresh, sign in to that account again. Encrypted keyring mode is not supported.",
            login: &["wrangler", "login"],
        },
        slots,
        identity: IdentityResolver::Command(WHOAMI),
        verify: Some(WHOAMI),
        extra_preflight: Some(preflight),
        usage: None,
        import_login: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_login_file_like_wrangler() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let mut mac = Cx::test(home, Os::Mac);
        assert_eq!(config_file(&mac), home.join("Library/Preferences/.wrangler/config/default.toml"));
        mac.set_env("XDG_CONFIG_HOME", &home.join("xdg").to_string_lossy());
        assert_eq!(config_file(&mac), home.join("xdg/.wrangler/config/default.toml"));
        mac.set_env("WRANGLER_API_ENVIRONMENT", "staging");
        assert_eq!(config_file(&mac), home.join("xdg/.wrangler/config/staging.toml"));

        assert_eq!(config_file(&Cx::test(home, Os::Linux)), home.join(".config/.wrangler/config/default.toml"));
        let mut windows = Cx::test(home, Os::Windows);
        windows.set_env("APPDATA", &home.join("Roaming").to_string_lossy());
        assert_eq!(config_file(&windows), home.join("Roaming/xdg.config/.wrangler/config/default.toml"));

        // The legacy folder wins while it exists, even without a login in it.
        std::fs::create_dir_all(home.join(".wrangler")).unwrap();
        assert_eq!(config_file(&Cx::test(home, Os::Mac)), home.join(".wrangler/config/default.toml"));
        assert_eq!(config_file(&windows), home.join(".wrangler/config/default.toml"));
    }

    #[test]
    fn preflight_flags_keyring_mode_and_profiles() {
        let dir = tempfile::tempdir().unwrap();
        let cx = Cx::test(dir.path(), Os::Linux);
        let config = dir.path().join(".config/.wrangler/config");
        std::fs::create_dir_all(&config).unwrap();
        std::fs::write(config.join("default.toml"), "oauth_token = \"x\"\n").unwrap();
        assert!(provider().preflight(&cx).is_empty());

        std::fs::write(config.join("default.enc"), "").unwrap();
        std::fs::write(config.join("work.toml"), "oauth_token = \"y\"\n").unwrap();
        let bindings = dir.path().join(".config/.wrangler/profiles");
        std::fs::create_dir_all(&bindings).unwrap();
        std::fs::write(
            bindings.join("directory-bindings.json"),
            r#"{"/code/client":"client","/code/client/api":"work"}"#,
        )
        .unwrap();
        let w = provider().preflight(&cx);
        assert!(w.iter().any(|w| w.code == "keyring-mode"));
        let profiles = w.iter().find(|w| w.code == "profiles").expect("profiles warning");
        assert!(profiles.message.contains("(client, work) and 2 folders bound to profiles"), "{}", profiles.message);
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
