//! Tables, prompts and JSON printing for the CLI.

use anyhow::{Result, bail};
use comfy_table::{Cell, ContentArrangement, Table, presets::UTF8_FULL_CONDENSED};

use crate::core::model::{ProviderInfo, ProviderStatus, Severity, TierInfo, Warning};
use crate::core::switch::Doctor;
use crate::core::{Core, Strategy};
use crate::providers::Provider;

pub fn json<T: serde::Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn table() -> Table {
    let mut t = Table::new();
    t.load_style(UTF8_FULL_CONDENSED).set_content_arrangement(ContentArrangement::Dynamic);
    t
}

fn tier_tag(t: &TierInfo) -> &'static str {
    match t {
        TierInfo::Supported => "",
        TierInfo::Experimental => " (experimental)",
        TierInfo::Unsupported { .. } => " (unsupported)",
    }
}

pub fn status_table(core: &Core, statuses: &[ProviderStatus]) {
    let hidden = core.settings.read().map(|s| s.hidden_providers.clone()).unwrap_or_default();
    let mut t = table();
    t.set_header(["Provider", "Logged in as", "Saved accounts", "Notes"]);
    let mut missing = Vec::new();
    for s in statuses {
        if hidden.contains(&s.info.id) {
            continue;
        }
        if s.installed.is_none() {
            missing.push(s.info.id.as_str());
            continue;
        }
        let live = match &s.live {
            Some(l) => {
                let mut txt = l.label.clone();
                if let Some(org) = l.extra.get("org").or_else(|| l.extra.get("plan")) {
                    txt.push_str(&format!("\n{org}"));
                }
                txt
            }
            None => "—".to_string(),
        };
        let accounts = if s.accounts.is_empty() {
            "(none saved)".to_string()
        } else {
            s.accounts
                .iter()
                .map(|a| {
                    let mark = if s.active_account.as_deref() == Some(&a.id) { "✓ " } else { "  " };
                    if a.label == a.id { format!("{mark}{}", a.id) } else { format!("{mark}{} ({})", a.label, a.id) }
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        let notes = s
            .warnings
            .iter()
            .map(|w| format!("{} {}", if w.severity == Severity::Warn { "!" } else { "i" }, w.message))
            .collect::<Vec<_>>()
            .join("\n");
        t.add_row([
            Cell::new(format!("{}{}\n{}", s.info.name, tier_tag(&s.info.tier), s.info.id)),
            Cell::new(live),
            Cell::new(accounts),
            Cell::new(notes),
        ]);
    }
    println!("{t}");
    if !missing.is_empty() {
        println!("Not detected on PATH: {}", missing.join(", "));
    }
}

pub fn providers_table(infos: &[ProviderInfo]) {
    let mut t = table();
    t.set_header(["Id", "Name", "Strategy", "Tier", "Notes"]);
    for i in infos {
        let tier = match &i.tier {
            TierInfo::Supported => "supported".to_string(),
            TierInfo::Experimental => "experimental".to_string(),
            TierInfo::Unsupported { reason } => format!("unsupported: {reason}"),
        };
        let strategy = match i.strategy {
            Strategy::SlotSwap => "slot swap",
            Strategy::NativeSwitch => "native switch",
        };
        t.add_row([&i.id, &i.name, strategy, &tier, &i.notes]);
    }
    println!("{t}");
}

pub fn accounts_table(statuses: &[ProviderStatus]) {
    let mut t = table();
    t.set_header(["Provider", "Account", "Label", "Active", "Last used"]);
    for s in statuses {
        for a in &s.accounts {
            t.add_row([
                s.info.id.clone(),
                a.id.clone(),
                a.label.clone(),
                if s.active_account.as_deref() == Some(&a.id) { "✓".to_string() } else { String::new() },
                a.last_used.map(|d| d.format("%Y-%m-%d %H:%M").to_string()).unwrap_or_default(),
            ]);
        }
    }
    println!("{t}");
}

pub fn print_warnings(warnings: &[Warning]) {
    for w in warnings {
        let tag = if w.severity == Severity::Warn { "warning" } else { "note" };
        println!("  {tag}: {}", w.message);
    }
}

pub fn doctor(d: &Doctor) {
    println!("Switcheroo {}  ({})", d.version, d.os);
    println!("data dir: {}", d.data_dir);
    match &d.vault_error {
        None => println!("vault:    {} (ok)", d.vault),
        Some(e) => println!("vault:    {} — {e}", d.vault),
    }
    println!();
    let mut t = table();
    t.set_header(["Provider", "Binary", "Version", "Logged in", "Slots", "Findings"]);
    for s in &d.providers {
        let (bin, ver) = match &s.installed {
            Some(i) => (i.path.display().to_string(), i.version.clone().unwrap_or_default()),
            None => ("not found".to_string(), String::new()),
        };
        let findings = s.warnings.iter().map(|w| format!("[{}] {}", w.code, w.message)).collect::<Vec<_>>().join("\n");
        t.add_row([
            format!("{}{}", s.info.id, tier_tag(&s.info.tier)),
            bin,
            ver,
            s.live.as_ref().map(|l| l.label.clone()).unwrap_or_else(|| "—".into()),
            s.slots.join("\n"),
            findings,
        ]);
    }
    println!("{t}");
}

pub fn pick_account(core: &Core, p: &dyn Provider) -> Result<String> {
    use std::io::IsTerminal;
    let status = core.status(p, false);
    if status.accounts.is_empty() {
        bail!(
            "no saved accounts for {}; run `switcheroo save {}` or `switcheroo login {}`",
            p.meta().name,
            p.meta().id,
            p.meta().id
        );
    }
    if !std::io::stdin().is_terminal() {
        bail!("specify an account: {}", status.accounts.iter().map(|a| a.id.as_str()).collect::<Vec<_>>().join(", "));
    }
    let items: Vec<String> = status
        .accounts
        .iter()
        .map(|a| {
            let active = if status.active_account.as_deref() == Some(&a.id) { "  (active)" } else { "" };
            if a.label == a.id { format!("{}{active}", a.id) } else { format!("{} — {}{active}", a.label, a.id) }
        })
        .collect();
    let idx = dialoguer::Select::new()
        .with_prompt(format!("Switch {} to", p.meta().name))
        .items(&items)
        .default(0)
        .interact()?;
    Ok(status.accounts[idx].id.clone())
}

pub fn confirm(prompt: &str) -> Result<bool> {
    use std::io::IsTerminal;
    if !std::io::stdin().is_terminal() {
        return Ok(true);
    }
    Ok(dialoguer::Confirm::new().with_prompt(prompt).default(false).interact()?)
}
