//! OpenAI Codex CLI (`codex`). Live slot: `$CODEX_HOME/auth.json` (file credential mode).
//! The account email and plan come from the `id_token` JWT inside that file.

use std::path::PathBuf;

use super::identity::IdentityResolver;
use super::slot_provider::SlotProvider;
use super::slots::{FileSlot, Slot};
use super::util::text::tilde;
use super::{Provider, ProviderMeta, Tier};
use crate::core::cx::Cx;
use crate::core::model::{Strategy, Warning};

fn codex_home(cx: &Cx) -> PathBuf {
    cx.env("CODEX_HOME").map(PathBuf::from).unwrap_or_else(|| cx.home().join(".codex"))
}

fn slots(cx: &Cx) -> Vec<Box<dyn Slot>> {
    let auth = codex_home(cx).join("auth.json");
    vec![Box::new(FileSlot::new(auth.clone(), tilde(&auth, cx.home())))]
}

fn preflight(cx: &Cx) -> Vec<Warning> {
    let mut w = Vec::new();
    let home = codex_home(cx);
    if let Ok(cfg) = std::fs::read_to_string(home.join("config.toml"))
        && cfg.lines().any(|l| {
            let l = l.trim();
            l.starts_with("cli_auth_credentials_store") && l.contains("keyring")
        })
    {
        w.push(Warning::warn(
            "keyring-mode",
            "config.toml sets cli_auth_credentials_store = \"keyring\"; Switcheroo only handles the auth.json file mode",
        ));
    }
    w
}

pub fn provider() -> Box<dyn Provider> {
    Box::new(SlotProvider {
        meta: ProviderMeta {
            id: "codex",
            name: "Codex CLI",
            strategy: Strategy::SlotSwap,
            tier: Tier::Supported,
            binaries: &["codex"],
            process_names: &["codex"],
            env_shadow: &["OPENAI_API_KEY"],
            restart_hint: Some(
                "Running Codex sessions and app-server daemons cache credentials at startup; restart them to use the new account.",
            ),
            notes: "Swaps ~/.codex/auth.json (honours CODEX_HOME). Keyring credential mode (cli_auth_credentials_store = \"keyring\") is not supported.",
            login: &["codex", "login"],
        },
        slots,
        identity: IdentityResolver::JwtClaim {
            slot: 0,
            token_pointer: "/tokens/id_token",
            claim: "email",
            extra: &[("plan", "/https:~1~1api.openai.com~1auth/chatgpt_plan_type")],
        },
        verify: None,
        extra_preflight: Some(preflight),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::cx::Os;
    use base64::Engine;

    fn jwt(claims: &str) -> String {
        let p = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(claims.as_bytes());
        format!("eyJhbGciOiJSUzI1NiJ9.{p}.sig")
    }

    #[test]
    fn email_and_plan_from_id_token() {
        let dir = tempfile::tempdir().unwrap();
        let cx = Cx::test(dir.path(), Os::Mac);
        std::fs::create_dir_all(dir.path().join(".codex")).unwrap();
        let token = jwt(r#"{"email":"Dev@Example.com","https://api.openai.com/auth":{"chatgpt_plan_type":"pro"}}"#);
        std::fs::write(
            dir.path().join(".codex/auth.json"),
            format!(r#"{{"OPENAI_API_KEY":null,"tokens":{{"id_token":"{token}","access_token":"a","refresh_token":"r","account_id":"acc"}},"last_refresh":"2026-09-08T00:00:00Z"}}"#),
        )
        .unwrap();
        let p = provider();
        let id = p.live_identity(&cx).unwrap().unwrap();
        assert_eq!(id.id, "dev@example.com");
        assert_eq!(id.extra["plan"], "pro");
        assert!(p.preflight(&cx).is_empty());
        std::fs::write(
            dir.path().join(".codex/config.toml"),
            "model = \"x\"\ncli_auth_credentials_store = \"keyring\"\n",
        )
        .unwrap();
        assert!(p.preflight(&cx).iter().any(|w| w.code == "keyring-mode"));
    }

    #[test]
    fn api_key_only_file_has_no_identity() {
        let dir = tempfile::tempdir().unwrap();
        let mut cx = Cx::test(dir.path(), Os::Linux);
        cx.set_env("CODEX_HOME", dir.path().join("cx").to_str().unwrap());
        std::fs::create_dir_all(dir.path().join("cx")).unwrap();
        std::fs::write(dir.path().join("cx/auth.json"), r#"{"OPENAI_API_KEY":"sk-x","tokens":null}"#).unwrap();
        let p = provider();
        assert!(p.live_identity(&cx).unwrap().is_none());
        let cap = p.capture(&cx).unwrap().unwrap();
        assert!(cap.identity.is_none());
    }
}
