//! Claude Code (`claude`). On macOS the Keychain item is used whenever it exists; the
//! credentials file is the slot only when the item is absent; `oauthAccount` moves with the
//! login; other keys in both stores stay. Claude Code hot-reloads credentials, so no restart
//! is needed.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use serde_json::Value;

use super::identity::{Cmd, IdentityResolver, Parse};
use super::slot_provider::SlotProvider;
#[cfg(target_os = "macos")]
use super::slots::KeychainJsonKeysSlot;
#[cfg(target_os = "macos")]
use super::slots::json_merge::classify_probe;
use super::slots::json_merge::{CredentialBackend, KeychainProbe, credential_backend};
use super::slots::{FileSlot, JsonKeysSlot, Slot};
use super::util::http;
use super::util::text::tilde;
use super::{Provider, ProviderMeta, Tier};
use crate::core::cx::{Cx, Os};
use crate::core::model::{Strategy, Usage, UsageItem, Warning};

/// Live login for the default config dir. Claude Code appends `-<hash>` only when
/// `CLAUDE_CONFIG_DIR` is set; that layout is already a preflight warning, and suffixed
/// items are out of scope.
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
    let credential_slot: Box<dyn Slot> = match choose_backend(cx) {
        CredentialBackend::File => file_credential_slot(cx, &creds),
        CredentialBackend::Keychain => keychain_credential_slot(cx, &creds),
    };
    let claude_json = cx.home().join(".claude.json");
    vec![
        credential_slot,
        Box::new(JsonKeysSlot::new(claude_json.clone(), &["oauthAccount"], tilde(&claude_json, cx.home()))),
    ]
}

fn file_credential_slot(cx: &Cx, creds: &Path) -> Box<dyn Slot> {
    Box::new(FileSlot::new(creds.to_path_buf(), tilde(creds, cx.home())))
}

#[cfg(target_os = "macos")]
fn keychain_credential_slot(cx: &Cx, _creds: &Path) -> Box<dyn Slot> {
    Box::new(KeychainJsonKeysSlot::new(KEYCHAIN_SERVICE, &keychain_account(cx), &["claudeAiOauth"]))
}

#[cfg(not(target_os = "macos"))]
fn keychain_credential_slot(cx: &Cx, creds: &Path) -> Box<dyn Slot> {
    // This build has no `security` binary. Backend selection already forces the file.
    file_credential_slot(cx, creds)
}

/// macOS probes the bare item only when commands are allowed (`Cx::test` never is). A non-mac
/// build cannot call `security`, so it treats the item as absent and uses the file.
fn choose_backend(cx: &Cx) -> CredentialBackend {
    match cx.os() {
        Os::Mac if cx.commands_allowed() => {
            #[cfg(target_os = "macos")]
            {
                credential_backend(true, probe_bare_item(cx))
            }
            #[cfg(not(target_os = "macos"))]
            {
                credential_backend(true, KeychainProbe::Absent)
            }
        }
        Os::Mac => credential_backend(true, KeychainProbe::Absent),
        _ => CredentialBackend::File,
    }
}

#[cfg(target_os = "macos")]
fn probe_bare_item(cx: &Cx) -> KeychainProbe {
    let found = crate::vault::macos_security::find_generic_password(KEYCHAIN_SERVICE, &keychain_account(cx));
    classify_probe(found)
}

#[cfg(target_os = "macos")]
fn keychain_account(cx: &Cx) -> String {
    cx.env("USER").map(str::to_string).unwrap_or_else(whoami)
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
    if let Some(warning) = credentials_file_warning(choose_backend(cx), credentials_file(cx).exists()) {
        w.push(warning);
    }
    w
}

fn credentials_file_warning(backend: CredentialBackend, file_exists: bool) -> Option<Warning> {
    if backend == CredentialBackend::Keychain && file_exists {
        Some(Warning::warn(
            "credentials-file",
            "~/.claude/.credentials.json is left untouched because the Keychain item exists",
        ))
    } else {
        None
    }
}

// Claude's OAuth usage endpoint is internal (the same one the Claude Code CLI uses); best effort.
const USAGE_URLS: &[&str] = &["https://api.anthropic.com/oauth/usage", "https://api.anthropic.com/api/oauth/usage"];
const USAGE_UA: &str = "claude-code/2.1.11";

fn usage(_cx: &Cx, slots: &[Option<Vec<u8>>]) -> Result<Option<Usage>> {
    let Some(bytes) = slots.first().and_then(|s| s.as_ref()) else { return Ok(None) };
    let creds: Value = serde_json::from_slice(bytes).context("credential is not JSON")?;
    let Some(token) = creds.pointer("/claudeAiOauth/accessToken").and_then(Value::as_str) else { return Ok(None) };
    let account: Option<Value> = slots.get(1).and_then(|s| s.as_deref()).and_then(|b| serde_json::from_slice(b).ok());
    let plan = plan_name(&creds, account.as_ref());
    if token_expired(&creds, Utc::now()) {
        return Ok(Some(expired_usage(plan.as_deref())));
    }
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

/// `expiresAt` is in milliseconds. Switcheroo does not renew the token itself, because a
/// renewal replaces the refresh token that Claude Code holds.
fn token_expired(creds: &Value, now: DateTime<Utc>) -> bool {
    creds
        .pointer("/claudeAiOauth/expiresAt")
        .and_then(Value::as_f64)
        .is_some_and(|ms| ms <= now.timestamp_millis() as f64)
}

/// Shown instead of calling the API with a token that has expired.
fn expired_usage(plan: Option<&str>) -> Usage {
    let mut expired = UsageItem::text("Usage", "Token expired");
    expired.detail = Some("Claude Code renews it the next time it runs".into());
    let mut items = vec![expired];
    if let Some(p) = plan.filter(|p| !p.is_empty()) {
        items.push(UsageItem::text("Plan", p));
    }
    Usage {
        items,
        note: Some("The access token of this login has expired, so Claude cannot be asked for usage. Claude Code renews the token the next time it runs; you do not need to sign in again.".into()),
        fetched_at: Utc::now(),
    }
}

fn iso(v: Option<&Value>) -> Option<DateTime<Utc>> {
    v.and_then(Value::as_str).and_then(|s| DateTime::parse_from_rfc3339(s).ok()).map(|d| d.with_timezone(&Utc))
}

fn title_case(words: &str) -> String {
    words
        .split('_')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Anthropic names plans on the server (`max` with the 20x tier is "Ultra"), and Claude Code
/// stores that name in `oauthAccount`. Older logins only have the subscription type and tier.
pub fn plan_name(creds: &Value, account: Option<&Value>) -> Option<String> {
    let display = account
        .and_then(|a| a.pointer("/oauthAccount/planDisplayName"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|n| !n.is_empty());
    if let Some(name) = display {
        return Some(name.to_string());
    }
    let kind = creds.pointer("/claudeAiOauth/subscriptionType").and_then(Value::as_str).filter(|s| !s.is_empty())?;
    let multiplier = creds
        .pointer("/claudeAiOauth/rateLimitTier")
        .and_then(Value::as_str)
        .and_then(|t| t.rsplit('_').next())
        .filter(|m| m.len() > 1 && m.ends_with('x') && m[..m.len() - 1].chars().all(|c| c.is_ascii_digit()));
    Some(match multiplier {
        Some(m) => format!("{} {m}", title_case(kind)),
        None => title_case(kind),
    })
}

/// `{"amount_minor": 1234, "currency": "USD", "exponent": 2}` as "$12.34".
fn money(v: Option<&Value>) -> Option<String> {
    let v = v?;
    let minor = v.get("amount_minor").and_then(Value::as_i64)?;
    let exponent = v.get("exponent").and_then(Value::as_u64).unwrap_or(2).min(6) as usize;
    let amount = minor as f64 / 10f64.powi(exponent as i32);
    Some(match v.get("currency").and_then(Value::as_str).unwrap_or("") {
        "USD" => format!("${amount:.exponent$}"),
        "" => format!("{amount:.exponent$}"),
        currency => format!("{amount:.exponent$} {currency}"),
    })
}

/// Extra usage past the plan limits. Newer responses send `spend` next to the older
/// `extra_usage`; `extra_usage` is used only when `spend` is absent.
fn extra_usage(v: &Value) -> Option<UsageItem> {
    if let Some(spend) = v.get("spend").filter(|s| s.is_object()) {
        if spend.get("enabled").and_then(Value::as_bool) != Some(true) {
            return None;
        }
        let used = money(spend.get("used"));
        return match (spend.get("percent").and_then(Value::as_f64), money(spend.get("limit"))) {
            (Some(pct), Some(limit)) => {
                let mut item = UsageItem::percent("Extra usage", pct, None);
                item.detail = used.map(|u| format!("{u} of {limit}"));
                Some(item)
            }
            _ => used.map(|u| UsageItem::text("Extra usage", format!("{u} spent"))),
        };
    }
    let extra = v.get("extra_usage").filter(|e| e.get("is_enabled").and_then(Value::as_bool) == Some(true))?;
    let pct = extra.get("utilization").and_then(Value::as_f64)?;
    Some(UsageItem::percent("Extra usage (monthly)", pct, None))
}

/// Which products used the weekly limit ("Claude Code 60% · Chats 5%"). The shares are of the
/// limit, so they add up to the weekly percent. Products at 0% are left out.
fn weekly_breakdown(v: &Value) -> Option<UsageItem> {
    let rows = v.pointer("/seven_day_breakdown/rows").and_then(Value::as_array)?;
    let mut parts: Vec<(&str, i64)> = rows
        .iter()
        .filter_map(|r| {
            let name = r.get("display_name").and_then(Value::as_str).map(str::trim).filter(|n| !n.is_empty())?;
            let pct = r.get("percent_of_limit").and_then(Value::as_f64)?.round() as i64;
            (pct > 0).then_some((name, pct))
        })
        .collect();
    if parts.is_empty() {
        return None;
    }
    parts.sort_by_key(|(_, pct)| std::cmp::Reverse(*pct));
    let value = parts.iter().map(|(name, pct)| format!("{name} {pct}%")).collect::<Vec<_>>().join(" · ");
    Some(UsageItem::text("Weekly breakdown", value))
}

/// Label for an entry of the structured `limits` array.
fn limit_label(kind: &str, scope: Option<&Value>) -> String {
    let scoped = scope
        .and_then(|s| s.pointer("/model/display_name").or_else(|| s.get("surface")))
        .and_then(Value::as_str)
        .filter(|n| !n.is_empty());
    let base = match kind {
        "session" => "Session (5h)".to_string(),
        "weekly_all" => "Weekly".to_string(),
        "weekly_scoped" => "Weekly".to_string(),
        other => title_case(other.trim_end_matches("_all").trim_end_matches("_scoped")),
    };
    match scoped {
        Some(name) => format!("{base} · {name}"),
        None => base,
    }
}

/// Anthropic's usage response. Prefer the structured `limits` array (session, weekly, per-model
/// weekly); fall back to the older `five_hour` / `seven_day` objects. Everything else in the
/// payload (internal feature buckets with codename keys) is ignored on purpose.
pub fn parse_claude_usage(v: &Value, plan: Option<&str>) -> Usage {
    let mut items = Vec::new();
    if let Some(limits) = v.get("limits").and_then(Value::as_array).filter(|l| !l.is_empty()) {
        for l in limits {
            let (Some(kind), Some(pct)) =
                (l.get("kind").and_then(Value::as_str), l.get("percent").and_then(Value::as_f64))
            else {
                continue;
            };
            let mut item = UsageItem::percent(limit_label(kind, l.get("scope")), pct, iso(l.get("resets_at")));
            if l.get("is_active").and_then(Value::as_bool) == Some(true) && pct >= 100.0 {
                item.detail = Some("limit reached".to_string());
            }
            items.push(item);
        }
    } else {
        for (key, label) in [("five_hour", "Session (5h)"), ("seven_day", "Weekly")] {
            if let Some(w) = v.get(key).filter(|w| w.is_object())
                && let Some(used) = w.get("utilization").and_then(Value::as_f64)
            {
                items.push(UsageItem::percent(label, used, iso(w.get("resets_at"))));
            }
        }
        for model in ["opus", "sonnet"] {
            if let Some(w) = v.get(format!("seven_day_{model}")).filter(|w| w.is_object())
                && let Some(used) = w.get("utilization").and_then(Value::as_f64)
            {
                items.push(UsageItem::percent(
                    format!("Weekly · {}", title_case(model)),
                    used,
                    iso(w.get("resets_at")),
                ));
            }
        }
    }
    items.extend(weekly_breakdown(v));
    items.extend(extra_usage(v));
    if let Some(p) = plan.filter(|p| !p.is_empty()) {
        items.push(UsageItem::text("Plan", p));
    }
    Usage {
        items,
        note: Some("Rolling limits as reported by Claude; the weekly windows are shared with claude.ai.".into()),
        fetched_at: Utc::now(),
    }
}

pub fn provider() -> Box<dyn Provider> {
    Box::new(SlotProvider {
        meta: ProviderMeta {
            id: "claude-code",
            color: "#D97757",
            name: "Claude Code",
            strategy: Strategy::SlotSwap,
            tier: Tier::Supported,
            binaries: &["claude"],
            process_names: &[],
            env_shadow: &["ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN", "CLAUDE_CODE_OAUTH_TOKEN"],
            restart_hint: None,
            notes: "On macOS the Keychain item \"Claude Code-credentials\" is used whenever it exists; ~/.claude/.credentials.json is the slot only when the item is absent. oauthAccount in ~/.claude.json moves with the login; other keys in both stores stay. Running sessions pick the new login up automatically.",
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
                    ("plan", "/oauthAccount/planDisplayName"),
                ],
            },
            IdentityResolver::Command(Cmd {
                argv: &["claude", "auth", "status"],
                parse: Parse::Json("/email"),
                is_email: true,
                extra: &[("org", "/orgName")],
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
        import_login: None,
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
            r#"{"numStartups":5,"userID":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","oauthAccount":{"accountUuid":"u1","emailAddress":"A@Example.com","organizationName":"Org A","organizationRole":"admin","billingType":"stripe_subscription","planDisplayName":"Ultra"},"projects":{"/x":{}}}"#,
        )
        .unwrap();
        let p = provider();
        let live = p.live_identity(&cx).unwrap().unwrap();
        assert_eq!(live.id, "a@example.com");
        assert_eq!(live.extra["org"], "Org A");
        assert_eq!(live.extra["plan"], "Ultra");
        let cap = p.capture(&cx).unwrap().unwrap();

        // switch to B by hand, then restore A: unrelated keys in ~/.claude.json survive
        std::fs::write(dir.path().join(".claude/.credentials.json"), r#"{"claudeAiOauth":{"accessToken":"B"}}"#)
            .unwrap();
        std::fs::write(
            dir.path().join(".claude.json"),
            r#"{"numStartups":6,"userID":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","oauthAccount":{"emailAddress":"b@example.com"},"projects":{"/x":{}}}"#,
        )
        .unwrap();
        assert_eq!(p.live_identity(&cx).unwrap().unwrap().id, "b@example.com");
        p.activate(&cx, &cap.secret, cap.identity.as_ref().unwrap()).unwrap();
        let cj: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.path().join(".claude.json")).unwrap()).unwrap();
        assert_eq!(cj["numStartups"], 6);
        assert_eq!(cj["projects"]["/x"], serde_json::json!({}));
        assert_eq!(cj["userID"], "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
        assert_eq!(cj["oauthAccount"]["emailAddress"], "A@Example.com");
        let creds = std::fs::read_to_string(dir.path().join(".claude/.credentials.json")).unwrap();
        assert!(creds.contains("sk-ant-oat01-A"));
        assert_eq!(p.slot_descriptions(&cx).len(), 2);
    }

    #[test]
    fn prefers_structured_limits_and_ignores_codename_buckets() {
        let v: Value = serde_json::from_str(
            r#"{"five_hour":{"utilization":100,"resets_at":"2026-09-08T22:00:00Z"},
                "nimbus_quill":{"utilization":0,"resets_at":null},
                "tangelo":null,
                "extra_usage":{"is_enabled":false,"utilization":null},
                "limits":[
                  {"kind":"session","group":"session","percent":100,"severity":"critical","resets_at":"2026-09-08T21:50:00+00:00","scope":null,"is_active":true},
                  {"kind":"weekly_all","group":"weekly","percent":19,"severity":"normal","resets_at":"2026-09-15T11:00:00+00:00","scope":null,"is_active":false},
                  {"kind":"weekly_scoped","group":"weekly","percent":37,"severity":"normal","resets_at":"2026-09-15T11:00:00+00:00","scope":{"model":{"id":null,"display_name":"Fable"},"surface":null},"is_active":false}
                ]}"#,
        )
        .unwrap();
        let u = parse_claude_usage(&v, Some("Ultra"));
        let labels: Vec<&str> = u.items.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(labels, ["Session (5h)", "Weekly", "Weekly · Fable", "Plan"]);
        assert_eq!(u.items[0].detail.as_deref(), Some("limit reached"));
        match &u.items[2].kind {
            crate::core::model::UsageKind::Percent { used, resets_at } => {
                assert_eq!(*used, 37.0);
                assert!(resets_at.is_some());
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    /// The response as of November 2026: no session window, `spend` next to `extra_usage`,
    /// and a weekly breakdown by surface.
    #[test]
    fn reads_the_current_response_shape() {
        let v: Value = serde_json::from_str(
            r#"{"five_hour":null,
                "seven_day":{"utilization":65.0,"resets_at":"2026-11-25T10:00:00.157629+00:00","limit_dollars":null,"used_dollars":null,"remaining_dollars":null,"locked_reason":null},
                "seven_day_opus":null,"seven_day_sonnet":null,"amber_ladder":null,"pace":null,
                "extra_usage":{"is_enabled":false,"monthly_limit":null,"used_credits":null,"utilization":null,"currency":null,"daily":null,"weekly":null},
                "limits":[
                  {"kind":"weekly_all","group":"weekly","percent":65,"severity":"normal","resets_at":"2026-11-25T10:00:00.157629+00:00","scope":null,"is_active":false},
                  {"kind":"weekly_scoped","group":"weekly","percent":78,"severity":"warning","resets_at":"2026-11-25T10:00:00.157758+00:00","scope":{"model":{"id":null,"display_name":"Fable"},"surface":null},"is_active":true}
                ],
                "spend":{"used":{"amount_minor":0,"currency":"USD","exponent":2},"limit":null,"percent":0,"severity":"normal","enabled":false,"cap":null,"balance":null},
                "seven_day_breakdown":{"rows":[{"key":"claude_code","display_name":"Claude Code","percent":100,"percent_of_limit":65}]},
                "weekly_scoped_shares":[{"limit_index":1,"used_percent_of_weekly":39,"allowance_percent_of_weekly":50}]}"#,
        )
        .unwrap();
        let u = parse_claude_usage(&v, Some("Ultra"));
        let rows: Vec<(String, String)> = u.items.iter().map(|i| (i.label.clone(), i.value_text())).collect();
        assert_eq!(
            rows,
            [
                ("Weekly".to_string(), "65% used".to_string()),
                ("Weekly · Fable".to_string(), "78% used".to_string()),
                ("Weekly breakdown".to_string(), "Claude Code 65%".to_string()),
                ("Plan".to_string(), "Ultra".to_string()),
            ]
        );
    }

    #[test]
    fn weekly_breakdown_lists_products_by_share() {
        let v: Value = serde_json::from_str(
            r#"{"seven_day_breakdown":{"rows":[
                {"key":"claude_code","display_name":"Claude Code","percent":20,"percent_of_limit":8},
                {"key":"chat","display_name":"Chats","percent":75,"percent_of_limit":30.4},
                {"key":"cowork","display_name":"Cowork","percent":5,"percent_of_limit":2},
                {"key":"other","display_name":"Other","percent":0,"percent_of_limit":0.2}
            ]}}"#,
        )
        .unwrap();
        let item = weekly_breakdown(&v).unwrap();
        assert_eq!(item.label, "Weekly breakdown");
        assert_eq!(item.value_text(), "Chats 30% · Claude Code 8% · Cowork 2%");

        let idle: Value = serde_json::from_str(
            r#"{"seven_day_breakdown":{"rows":[{"key":"chat","display_name":"Chats","percent":0,"percent_of_limit":0}]}}"#,
        )
        .unwrap();
        assert!(weekly_breakdown(&idle).is_none());
        assert!(weekly_breakdown(&serde_json::json!({"seven_day_breakdown": null})).is_none());
    }

    #[test]
    fn extra_usage_comes_from_spend() {
        let with_limit: Value = serde_json::from_str(
            r#"{"spend":{"enabled":true,"percent":25,"used":{"amount_minor":1250,"currency":"USD","exponent":2},"limit":{"amount_minor":5000,"currency":"USD","exponent":2}},
                "extra_usage":{"is_enabled":true,"utilization":99}}"#,
        )
        .unwrap();
        let item = extra_usage(&with_limit).unwrap();
        assert_eq!(item.label, "Extra usage");
        assert_eq!(item.value_text(), "25% used");
        assert_eq!(item.detail.as_deref(), Some("$12.50 of $50.00"));

        let no_limit: Value =
            serde_json::from_str(r#"{"spend":{"enabled":true,"percent":0,"used":{"amount_minor":700,"currency":"EUR","exponent":2},"limit":null}}"#)
                .unwrap();
        assert_eq!(extra_usage(&no_limit).unwrap().value_text(), "7.00 EUR spent");

        let disabled: Value =
            serde_json::from_str(r#"{"spend":{"enabled":false},"extra_usage":{"is_enabled":true,"utilization":40}}"#)
                .unwrap();
        assert!(extra_usage(&disabled).is_none());

        let legacy: Value = serde_json::from_str(r#"{"extra_usage":{"is_enabled":true,"utilization":40}}"#).unwrap();
        let item = extra_usage(&legacy).unwrap();
        assert_eq!(item.label, "Extra usage (monthly)");
        assert_eq!(item.value_text(), "40% used");
    }

    #[test]
    fn plan_name_prefers_the_server_name() {
        let creds: Value = serde_json::from_str(
            r#"{"claudeAiOauth":{"subscriptionType":"max","rateLimitTier":"default_claude_max_20x"}}"#,
        )
        .unwrap();
        let account: Value = serde_json::from_str(r#"{"oauthAccount":{"planDisplayName":"Ultra"}}"#).unwrap();
        assert_eq!(plan_name(&creds, Some(&account)).as_deref(), Some("Ultra"));
        let unnamed: Value = serde_json::from_str(r#"{"oauthAccount":{"planDisplayName":" "}}"#).unwrap();
        assert_eq!(plan_name(&creds, Some(&unnamed)).as_deref(), Some("Max 20x"));
        assert_eq!(plan_name(&creds, None).as_deref(), Some("Max 20x"));

        let pro: Value =
            serde_json::from_str(r#"{"claudeAiOauth":{"subscriptionType":"pro","rateLimitTier":"default_claude_ai"}}"#)
                .unwrap();
        assert_eq!(plan_name(&pro, None).as_deref(), Some("Pro"));
        assert_eq!(plan_name(&serde_json::json!({}), None), None);
    }

    #[test]
    fn an_expired_token_is_not_sent() {
        let now = DateTime::parse_from_rfc3339("2026-11-23T16:00:00Z").unwrap().with_timezone(&Utc);
        let creds = |ms: i64| serde_json::json!({"claudeAiOauth": {"accessToken": "x", "expiresAt": ms}});
        assert!(token_expired(&creds(now.timestamp_millis() - 1), now));
        assert!(token_expired(&creds(now.timestamp_millis()), now));
        assert!(!token_expired(&creds(now.timestamp_millis() + 60_000), now));
        assert!(!token_expired(&serde_json::json!({"claudeAiOauth": {"accessToken": "x"}}), now));

        let u = expired_usage(Some("Ultra"));
        let rows: Vec<(&str, String)> = u.items.iter().map(|i| (i.label.as_str(), i.value_text())).collect();
        assert_eq!(rows, [("Usage", "Token expired".to_string()), ("Plan", "Ultra".to_string())]);
        assert_eq!(u.items[0].detail.as_deref(), Some("Claude Code renews it the next time it runs"));
    }

    #[test]
    fn falls_back_to_legacy_windows() {
        let v: Value = serde_json::from_str(
            r#"{"five_hour":{"utilization":42.5,"resets_at":"2026-09-08T22:00:00Z"},
                "seven_day":{"utilization":12,"resets_at":"2026-09-12T00:00:00+00:00"},
                "seven_day_opus":{"utilization":3.2,"resets_at":"2026-09-12T00:00:00Z"},
                "iguana_necktie":{"utilization":0}}"#,
        )
        .unwrap();
        let u = parse_claude_usage(&v, None);
        let labels: Vec<&str> = u.items.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(labels, ["Session (5h)", "Weekly", "Weekly · Opus"]);
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

    #[test]
    fn linux_preflight_does_not_warn_about_the_credentials_file() {
        let dir = tempfile::tempdir().unwrap();
        let cx = Cx::test(dir.path(), Os::Linux);
        std::fs::create_dir_all(dir.path().join(".claude")).unwrap();
        std::fs::write(dir.path().join(".claude/.credentials.json"), "{}").unwrap();
        let w = provider().preflight(&cx);
        assert!(w.iter().all(|w| w.code != "credentials-file"));
    }

    #[test]
    fn credentials_file_warning_only_when_keychain_owns_the_login() {
        let warning = credentials_file_warning(CredentialBackend::Keychain, true).expect("warning");
        assert_eq!(warning.code, "credentials-file");
        assert!(
            warning.message.contains("left untouched") && warning.message.contains("Keychain item exists"),
            "{}",
            warning.message
        );
        assert!(credentials_file_warning(CredentialBackend::File, true).is_none());
        assert!(credentials_file_warning(CredentialBackend::Keychain, false).is_none());
    }
}
