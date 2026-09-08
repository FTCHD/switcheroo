//! Vercel CLI (`vercel`). Live slot: `auth.json` → `token` in the global config dir, plus
//! `config.json` → `currentTeam` so the selected team scope travels with the account.

use std::path::PathBuf;

use super::identity::{Cmd, IdentityResolver, Parse};
use super::slot_provider::SlotProvider;
use super::slots::{JsonKeysSlot, Slot};
use super::util::text::tilde;
use super::{Provider, ProviderMeta, Tier};
use crate::core::cx::{Cx, Os};
use crate::core::model::{Strategy, Warning};

fn global_dir(cx: &Cx) -> PathBuf {
    match cx.os() {
        Os::Windows => cx.config_dir().join("xdg.data").join("com.vercel.cli"),
        _ => cx.data_dir().join("com.vercel.cli"),
    }
}

fn slots(cx: &Cx) -> Vec<Box<dyn Slot>> {
    let dir = global_dir(cx);
    let auth = dir.join("auth.json");
    let config = dir.join("config.json");
    vec![
        Box::new(JsonKeysSlot::new(auth.clone(), &["token"], tilde(&auth, cx.home()))),
        Box::new(JsonKeysSlot::new(config.clone(), &["currentTeam"], tilde(&config, cx.home()))),
    ]
}

fn preflight(cx: &Cx) -> Vec<Warning> {
    let mut w = Vec::new();
    let keyring_env = cx.env("VERCEL_TOKEN_STORAGE").is_some_and(|v| v.eq_ignore_ascii_case("keyring"));
    let keyring_cfg = std::fs::read(global_dir(cx).join("config.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .and_then(|v| v.get("credStorage").and_then(|s| s.as_str()).map(|s| s.eq_ignore_ascii_case("keyring")))
        .unwrap_or(false);
    if keyring_env || keyring_cfg {
        w.push(Warning::warn(
            "keyring-mode",
            "Vercel is configured to keep its token in the OS keyring; Switcheroo only handles the auth.json file mode",
        ));
    }
    w
}

const WHOAMI: Cmd = Cmd { argv: &["vercel", "whoami"], parse: Parse::Trim, is_email: false, extra: &[] };

pub fn provider() -> Box<dyn Provider> {
    Box::new(SlotProvider {
        meta: ProviderMeta {
            id: "vercel",
            color: "#0070F3",
            name: "Vercel",
            strategy: Strategy::SlotSwap,
            tier: Tier::Supported,
            binaries: &["vercel"],
            process_names: &[],
            env_shadow: &["VERCEL_TOKEN"],
            restart_hint: None,
            notes: "Swaps the token in the global auth.json and the currentTeam scope in config.json. The username comes from `vercel whoami` (network) when a login is saved.",
            login: &["vercel", "login"],
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
    fn paths_per_os_and_keyring_detection() {
        let dir = tempfile::tempdir().unwrap();
        let cx = Cx::test(dir.path(), Os::Mac);
        assert_eq!(global_dir(&cx), dir.path().join("Library/Application Support/com.vercel.cli"));
        let lx = Cx::test(dir.path(), Os::Linux);
        assert_eq!(global_dir(&lx), dir.path().join(".local/share/com.vercel.cli"));
        let d = global_dir(&lx);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("config.json"), r#"{"credStorage":"keyring"}"#).unwrap();
        assert!(provider().preflight(&lx).iter().any(|w| w.code == "keyring-mode"));
    }

    #[test]
    fn slots_capture_token_and_team_only() {
        let dir = tempfile::tempdir().unwrap();
        let cx = Cx::test(dir.path(), Os::Linux);
        let d = global_dir(&cx);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("auth.json"), r#"{"// Note":"x","token":"tok-a"}"#).unwrap();
        std::fs::write(d.join("config.json"), r#"{"collectMetrics":true,"currentTeam":"team_1"}"#).unwrap();
        let p = provider();
        let cap = p.capture(&cx).unwrap().unwrap();
        assert!(cap.identity.is_none(), "identity needs the network; none in tests");
        std::fs::write(d.join("auth.json"), r#"{"// Note":"x","token":"tok-b"}"#).unwrap();
        std::fs::write(d.join("config.json"), r#"{"collectMetrics":false}"#).unwrap();
        p.activate(&cx, &cap.secret, &crate::core::model::Identity::new("me")).unwrap();
        let auth: serde_json::Value = serde_json::from_slice(&std::fs::read(d.join("auth.json")).unwrap()).unwrap();
        let cfg: serde_json::Value = serde_json::from_slice(&std::fs::read(d.join("config.json")).unwrap()).unwrap();
        assert_eq!(auth["token"], "tok-a");
        assert_eq!(cfg["currentTeam"], "team_1");
        assert_eq!(cfg["collectMetrics"], false);
    }
}
