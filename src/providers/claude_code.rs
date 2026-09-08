//! Claude Code (`claude`). Live slot: the OAuth blob (macOS keychain item
//! `Claude Code-credentials` / `~/.claude/.credentials.json` elsewhere) plus the `oauthAccount`
//! key of `~/.claude.json`, which Claude Code shows as the logged-in account. Both move together.
//! Claude Code hot-reloads credentials, so no restart is needed.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use serde_json::Value;

use super::identity::{Cmd, IdentityResolver, Parse};
use super::slot_provider::SlotProvider;
use super::slots::{FileSlot, JsonKeysSlot, Slot};
use super::util::http;
use super::util::text::tilde;
use super::{Provider, ProviderMeta, Tier};
use crate::core::cx::{Cx, Os};
use crate::core::model::{Strategy, Usage, UsageItem, Warning};

#[cfg(target_os = "macos")]
pub const KEYCHAIN_SERVICE: &str = "Claude Code-credentials";

fn config_dir(cx: &Cx) -> PathBuf {
    cx.env("CLAUDE_CONFIG_DIR").map(PathBuf::from).unwrap_or_else(|| cx.home().join(".claude"))
}

fn credentials_file(cx: &Cx) -> PathBuf {
    config_dir(cx).join(".credentials.json")
}

fn slots(cx: &Cx) -> Vec<Box<dyn Slot>> {
    let creds = credentials_file(cx);
    let credential_slot: Box<dyn Slot> = if cx.os() == Os::Mac && !creds.exists() {
        #[cfg(target_os = "macos")]
        {
            let account = cx.env("USER").map(str::to_string).unwrap_or_else(whoami);
            Box::new(super::slots::KeychainItemSlot::new(KEYCHAIN_SERVICE, &account))
        }
        #[cfg(not(target_os = "macos"))]
        {
            Box::new(FileSlot::new(creds.clone(), tilde(&creds, cx.home())))
        }
    } else {
        Box::new(FileSlot::new(creds.clone(), tilde(&creds, cx.home())))
    };
    let claude_json = cx.home().join(".claude.json");
    vec![
        credential_slot,
        Box::new(JsonKeysSlot::new(claude_json.clone(), &["oauthAccount"], tilde(&claude_json, cx.home()))),
    ]
}

#[cfg(target_os = "macos")]
fn whoami() -> String {
    std::process::Command::new("/usr/bin/id")
        .arg("-un")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

fn preflight(cx: &Cx) -> Vec<Warning> {
    let mut w = Vec::new();
    if cx.env("CLAUDE_CONFIG_DIR").is_some() {
        w.push(Warning::warn(
            "config-dir",
            "CLAUDE_CONFIG_DIR is set; Claude Code keys its keychain entry to that directory, which Switcheroo does not support",
        ));
    }
    w
}

// Claude's OAuth usage endpoint is internal (the same one the Claude Code CLI uses); best effort.
const USAGE_URLS: &[&str] = &["https://api.anthropic.com/oauth/usage", "https://api.anthropic.com/api/oauth/usage"];
const USAGE_UA: &str = "claude-code/2.1.11";

fn usage(_cx: &Cx, slots: &[Option<Vec<u8>>]) -> Result<Option<Usage>> {
    let Some(bytes) = slots.first().and_then(|s| s.as_ref()) else { return Ok(None) };
    let creds: Value = serde_json::from_slice(bytes).context("credential is not JSON")?;
    let Some(token) = creds.pointer("/claudeAiOauth/accessToken").and_then(Value::as_str) else { return Ok(None) };
    let plan = creds.pointer("/claudeAiOauth/subscriptionType").and_then(Value::as_str).map(str::to_string);
    let auth = format!("Bearer {token}");
    let headers = [
        ("Authorization", auth.as_str()),
        ("anthropic-beta", "oauth-2025-04-20"),
        ("Accept", "application/json"),
        ("User-Agent", USAGE_UA),
    ];
    let mut last = None;
    for url in USAGE_URLS {
        let resp = http::get_json(url, &headers)?;
        match resp.status {
            404 => {
                last = Some(format!("HTTP 404 at {url}"));
                continue;
            }
            401 | 403 => bail!("Claude rejected the token; sign in again"),
            _ => {}
        }
        let json = http::ensure_ok(&resp, "Claude usage")?;
        return Ok(Some(parse_claude_usage(json, plan.as_deref())));
    }
    bail!("Claude usage endpoint not found ({})", last.unwrap_or_default())
}

fn iso(v: Option<&Value>) -> Option<DateTime<Utc>> {
    v.and_then(Value::as_str).and_then(|s| DateTime::parse_from_rfc3339(s).ok()).map(|d| d.with_timezone(&Utc))
}

fn window_label(key: &str) -> String {
    let (base, rest) = if let Some(r) = key.strip_prefix("seven_day") {
        ("Weekly", r)
    } else if let Some(r) = key.strip_prefix("five_hour") {
        ("Session (5h)", r)
    } else {
        ("", key)
    };
    let suffix: Vec<String> = rest
        .split('_')
        .filter(|s| !s.is_empty())
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect();
    match (base.is_empty(), suffix.is_empty()) {
        (false, true) => base.to_string(),
        (false, false) => format!("{base} · {}", suffix.join(" ")),
        (true, _) => suffix.join(" "),
    }
}

/// `five_hour`/`seven_day` (and any other window object with a `utilization`) → percent meters.
pub fn parse_claude_usage(v: &Value, plan: Option<&str>) -> Usage {
    let mut items = Vec::new();
    let mut seen = Vec::new();
    for key in ["five_hour", "seven_day"] {
        if let Some(w) = v.get(key).filter(|w| w.is_object()) {
            seen.push(key.to_string());
            if let Some(used) = w.get("utilization").and_then(Value::as_f64) {
                items.push(UsageItem::percent(window_label(key), used, iso(w.get("resets_at"))));
            }
        }
    }
    if let Some(obj) = v.as_object() {
        for (key, w) in obj {
            if seen.contains(key) || !w.is_object() {
                continue;
            }
            if let Some(used) = w.get("utilization").and_then(Value::as_f64) {
                items.push(UsageItem::percent(window_label(key), used, iso(w.get("resets_at"))));
            }
        }
    }
    if let Some(p) = plan.filter(|p| !p.is_empty()) {
        let mut c = p.chars();
        let pretty = match c.next() {
            Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            None => String::new(),
        };
        items.push(UsageItem::text("Plan", pretty));
    }
    Usage {
        items,
        note: Some("Rolling limits as reported by Claude; the weekly window is shared with claude.ai.".into()),
        fetched_at: Utc::now(),
    }
}

pub fn provider() -> Box<dyn Provider> {
    Box::new(SlotProvider {
        meta: ProviderMeta {
            id: "claude-code",
            name: "Claude Code",
            strategy: Strategy::SlotSwap,
            tier: Tier::Supported,
            binaries: &["claude"],
            process_names: &[],
            env_shadow: &["ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN", "CLAUDE_CODE_OAUTH_TOKEN"],
            restart_hint: None,
            notes: "Swaps the OAuth credential (macOS keychain item \"Claude Code-credentials\", or ~/.claude/.credentials.json) together with the oauthAccount entry in ~/.claude.json. Running sessions pick the new login up automatically.",
            login: &["claude", "auth", "login"],
        },
        slots,
        identity: IdentityResolver::Chain(&[
            IdentityResolver::JsonPointer {
                slot: 1,
                pointer: "/oauthAccount/emailAddress",
                is_email: true,
                extra: &[
                    ("org", "/oauthAccount/organizationName"),
                    ("role", "/oauthAccount/organizationRole"),
                    ("billing", "/oauthAccount/billingType"),
                ],
            },
            IdentityResolver::Command(Cmd {
                argv: &["claude", "auth", "status"],
                parse: Parse::Json("/email"),
                is_email: true,
                extra: &[("org", "/orgName"), ("plan", "/subscriptionType")],
            }),
        ]),
        verify: Some(Cmd {
            argv: &["claude", "auth", "status"],
            parse: Parse::Json("/email"),
            is_email: true,
            extra: &[],
        }),
        extra_preflight: Some(preflight),
        usage: Some(usage),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_comes_from_claude_json_and_both_slots_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let cx = Cx::test(dir.path(), Os::Linux);
        std::fs::create_dir_all(dir.path().join(".claude")).unwrap();
        std::fs::write(
            dir.path().join(".claude/.credentials.json"),
            r#"{"claudeAiOauth":{"accessToken":"sk-ant-oat01-A","refreshToken":"sk-ant-ort01-A","expiresAt":1,"scopes":["user:inference"],"subscriptionType":"max"}}"#,
        )
        .unwrap();
        std::fs::write(
            dir.path().join(".claude.json"),
            r#"{"numStartups":5,"oauthAccount":{"accountUuid":"u1","emailAddress":"A@Example.com","organizationName":"Org A","organizationRole":"admin","billingType":"stripe_subscription"},"projects":{"/x":{}}}"#,
        )
        .unwrap();
        let p = provider();
        let live = p.live_identity(&cx).unwrap().unwrap();
        assert_eq!(live.id, "a@example.com");
        assert_eq!(live.extra["org"], "Org A");
        let cap = p.capture(&cx).unwrap().unwrap();

        // switch to B by hand, then restore A: unrelated keys in ~/.claude.json survive
        std::fs::write(dir.path().join(".claude/.credentials.json"), r#"{"claudeAiOauth":{"accessToken":"B"}}"#)
            .unwrap();
        std::fs::write(
            dir.path().join(".claude.json"),
            r#"{"numStartups":6,"oauthAccount":{"emailAddress":"b@example.com"}}"#,
        )
        .unwrap();
        assert_eq!(p.live_identity(&cx).unwrap().unwrap().id, "b@example.com");
        p.activate(&cx, &cap.secret, cap.identity.as_ref().unwrap()).unwrap();
        let cj: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.path().join(".claude.json")).unwrap()).unwrap();
        assert_eq!(cj["numStartups"], 6);
        assert_eq!(cj["oauthAccount"]["emailAddress"], "A@Example.com");
        let creds = std::fs::read_to_string(dir.path().join(".claude/.credentials.json")).unwrap();
        assert!(creds.contains("sk-ant-oat01-A"));
        assert_eq!(p.slot_descriptions(&cx).len(), 2);
    }

    #[test]
    fn parses_usage_windows_and_extra_models() {
        let v: Value = serde_json::from_str(
            r#"{"five_hour":{"utilization":42.5,"resets_at":"2026-09-08T22:00:00Z"},
                "seven_day":{"utilization":12,"resets_at":"2026-09-12T00:00:00+00:00"},
                "seven_day_opus":{"utilization":3.2,"resets_at":"2026-09-12T00:00:00Z"},
                "extra_usage":{"enabled":false}}"#,
        )
        .unwrap();
        let u = parse_claude_usage(&v, Some("max"));
        let labels: Vec<&str> = u.items.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(labels, ["Session (5h)", "Weekly", "Weekly · Opus", "Plan"]);
        match &u.items[0].kind {
            crate::core::model::UsageKind::Percent { used, resets_at } => {
                assert_eq!(*used, 42.5);
                assert!(resets_at.is_some());
            }
            other => panic!("unexpected {other:?}"),
        }
        match &u.items[3].kind {
            crate::core::model::UsageKind::Text { value } => assert_eq!(value, "Max"),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn config_dir_env_is_flagged() {
        let dir = tempfile::tempdir().unwrap();
        let mut cx = Cx::test(dir.path(), Os::Linux);
        cx.set_env("CLAUDE_CONFIG_DIR", "/elsewhere");
        let w = provider().preflight(&cx);
        assert!(w.iter().any(|w| w.code == "config-dir"));
        assert_eq!(credentials_file(&cx), PathBuf::from("/elsewhere/.credentials.json"));
    }
}
