//! OpenAI Codex CLI (`codex`). Live slot: `$CODEX_HOME/auth.json` (file credential mode).
//! The account email and plan come from the `id_token` JWT inside that file.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use serde_json::Value;

use super::identity::IdentityResolver;
use super::slot_provider::SlotProvider;
use super::slots::{FileSlot, Slot};
use super::util::text::tilde;
use super::util::{http, jwt};
use super::{Provider, ProviderMeta, Tier};
use crate::core::cx::Cx;
use crate::core::model::{Strategy, Usage, UsageItem, Warning};

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

// ChatGPT's internal usage endpoint, as used by community switchers; best effort.
const USAGE_URLS: &[&str] =
    &["https://chatgpt.com/backend-api/wham/usage", "https://chatgpt.com/backend-api/api/codex/usage"];

fn usage(_cx: &Cx, slots: &[Option<Vec<u8>>]) -> Result<Option<Usage>> {
    let Some(bytes) = slots.first().and_then(|s| s.as_ref()) else { return Ok(None) };
    let auth: Value = serde_json::from_slice(bytes).context("auth.json is not JSON")?;
    let Some(token) = auth.pointer("/tokens/access_token").and_then(Value::as_str) else { return Ok(None) };
    let account_id = auth
        .pointer("/tokens/account_id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            auth.pointer("/tokens/id_token").and_then(Value::as_str).and_then(|t| jwt::claims(t).ok()).and_then(|c| {
                c.pointer("/https:~1~1api.openai.com~1auth/chatgpt_account_id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
        })
        .unwrap_or_default();
    let bearer = format!("Bearer {token}");
    let ua = format!("switcheroo/{}", env!("CARGO_PKG_VERSION"));
    let headers = [
        ("Authorization", bearer.as_str()),
        ("ChatGPT-Account-Id", account_id.as_str()),
        ("Accept", "application/json"),
        ("User-Agent", ua.as_str()),
    ];
    for url in USAGE_URLS {
        let resp = http::get_json(url, &headers)?;
        match resp.status {
            404 => continue,
            401 | 403 => bail!("ChatGPT rejected the token; sign in again"),
            _ => {}
        }
        let json = http::ensure_ok(&resp, "Codex usage")?;
        return Ok(Some(parse_codex_usage(json)));
    }
    bail!("Codex usage endpoint not found")
}

fn window_label(secs: Option<f64>, fallback: &str) -> String {
    match secs {
        Some(s) if (7.0 * 86_400.0 - 1.0..=7.0 * 86_400.0 + 1.0).contains(&s) => "Weekly".to_string(),
        Some(s) if s >= 86_400.0 => format!("{}d window", (s / 86_400.0).round() as i64),
        Some(s) if s >= 3600.0 => format!("Session ({}h)", (s / 3600.0).round() as i64),
        Some(s) if s > 0.0 => format!("{}m window", (s / 60.0).round() as i64),
        _ => fallback.to_string(),
    }
}

/// `rate_limit.primary_window` / `secondary_window` → percent meters; `plan_type` → text.
pub fn parse_codex_usage(v: &Value) -> Usage {
    let rl = v.get("rate_limit").filter(|r| r.is_object()).unwrap_or(v);
    let mut items = Vec::new();
    for (key, fallback) in [("primary_window", "Primary limit"), ("secondary_window", "Secondary limit")] {
        let Some(w) = rl.get(key).filter(|w| w.is_object()) else { continue };
        let Some(used) = w.get("used_percent").and_then(Value::as_f64) else { continue };
        let resets_at = w.get("reset_at").and_then(Value::as_f64).and_then(|t| DateTime::from_timestamp(t as i64, 0));
        let secs = w.get("limit_window_seconds").or_else(|| w.get("window_seconds")).and_then(Value::as_f64);
        items.push(UsageItem::percent(window_label(secs, fallback), used, resets_at));
    }
    if let Some(plan) = v.get("plan_type").and_then(Value::as_str).filter(|p| !p.is_empty()) {
        let mut c = plan.chars();
        let pretty = match c.next() {
            Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            None => String::new(),
        };
        items.push(UsageItem::text("Plan", pretty));
    }
    Usage { items, note: Some("Rate limits as reported by ChatGPT for this account.".into()), fetched_at: Utc::now() }
}

pub fn provider() -> Box<dyn Provider> {
    Box::new(SlotProvider {
        meta: ProviderMeta {
            id: "codex",
            color: "#10A37F",
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
        usage: Some(usage),
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
    fn parses_codex_rate_limits() {
        let v: Value = serde_json::from_str(
            r#"{"plan_type":"plus","rate_limit":{
                "primary_window":{"used_percent":45,"reset_at":1789000000,"limit_window_seconds":18000},
                "secondary_window":{"used_percent":12.5,"reset_at":1789400000,"limit_window_seconds":604800}}}"#,
        )
        .unwrap();
        let u = parse_codex_usage(&v);
        let labels: Vec<&str> = u.items.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(labels, ["Session (5h)", "Weekly", "Plan"]);
        let v2: Value = serde_json::from_str(r#"{"rate_limit":{"primary_window":{"used_percent":1}}}"#).unwrap();
        assert_eq!(parse_codex_usage(&v2).items[0].label, "Primary limit");
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
