//! The tray menu, rebuilt wholesale from provider statuses. Menu item ids encode the action
//! (`switch:<provider>:<account>`), so no id → action map has to be kept in sync.

use tray_icon::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};

use crate::core::model::{ProviderStatus, TierInfo};
use crate::core::settings::Settings;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Open,
    Refresh,
    Quit,
    Switch {
        provider: String,
        account: String,
    },
    Save {
        provider: String,
    },
    Login {
        provider: String,
    },
    /// Toggle start-at-login to `enable`.
    Autostart {
        enable: bool,
    },
    None,
}

impl Action {
    pub fn parse(id: &str) -> Action {
        let mut parts = id.splitn(3, ':');
        match (parts.next(), parts.next(), parts.next()) {
            (Some("open"), _, _) => Action::Open,
            (Some("refresh"), _, _) => Action::Refresh,
            (Some("quit"), _, _) => Action::Quit,
            (Some("switch"), Some(p), Some(a)) => Action::Switch { provider: p.to_string(), account: a.to_string() },
            (Some("save"), Some(p), _) => Action::Save { provider: p.to_string() },
            (Some("login"), Some(p), _) => Action::Login { provider: p.to_string() },
            (Some("autostart"), Some("on"), _) => Action::Autostart { enable: false },
            (Some("autostart"), Some("off"), _) => Action::Autostart { enable: true },
            _ => Action::None,
        }
    }
}

pub fn loading_menu() -> Menu {
    let menu = Menu::new();
    let _ = menu.append(&MenuItem::with_id("noop", "Detecting CLIs…", false, None));
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&MenuItem::with_id("open", "Open Switcheroo…", true, None));
    let _ = menu.append(&MenuItem::with_id("quit", "Quit Switcheroo", true, None));
    menu
}

pub fn build(statuses: &[ProviderStatus], settings: &Settings, autostart: bool, last_error: Option<&str>) -> Menu {
    let menu = Menu::new();
    if let Some(err) = last_error {
        let _ = menu.append(&MenuItem::with_id("noop", format!("⚠ {}", truncate(err, 80)), false, None));
        let _ = menu.append(&PredefinedMenuItem::separator());
    }
    let mut shown = 0;
    for s in statuses {
        if s.installed.is_none()
            || settings.hidden_providers.contains(&s.info.id)
            || matches!(s.info.tier, TierInfo::Unsupported { .. })
        {
            continue;
        }
        shown += 1;
        let live = s.live.as_ref().map(|l| display_name(&l.label, &l.id, settings.tray.show_emails));
        let title = match &live {
            Some(l) => format!("{} — {}", s.info.name, l),
            None => format!("{} — not logged in", s.info.name),
        };
        let sub = Submenu::new(title, true);
        if s.accounts.is_empty() {
            let _ = sub.append(&MenuItem::with_id("noop", "No saved accounts", false, None));
        }
        for a in &s.accounts {
            let active = s.active_account.as_deref() == Some(&a.id);
            let text = display_name(&a.label, &a.id, settings.tray.show_emails);
            let _ = sub.append(&CheckMenuItem::with_id(
                format!("switch:{}:{}", s.info.id, a.id),
                text,
                !active,
                active,
                None,
            ));
        }
        let _ = sub.append(&PredefinedMenuItem::separator());
        if s.live.is_some() && s.active_account.is_none() {
            let _ = sub.append(&MenuItem::with_id(format!("save:{}", s.info.id), "Save current login", true, None));
        }
        let _ = sub.append(&MenuItem::with_id(format!("login:{}", s.info.id), "Add account…", true, None));
        for w in &s.warnings {
            let _ = sub.append(&MenuItem::with_id("noop", format!("⚠ {}", truncate(&w.message, 70)), false, None));
        }
        let _ = menu.append(&sub);
    }
    if shown == 0 {
        let _ = menu.append(&MenuItem::with_id("noop", "No supported CLIs detected", false, None));
    }
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&MenuItem::with_id("open", "Open Switcheroo…", true, None));
    let _ = menu.append(&MenuItem::with_id("refresh", "Refresh", true, None));
    let _ = menu.append(&CheckMenuItem::with_id(
        if autostart { "autostart:on" } else { "autostart:off" },
        "Start at login",
        true,
        autostart,
        None,
    ));
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&MenuItem::with_id("quit", "Quit Switcheroo", true, None));
    menu
}

fn display_name(label: &str, id: &str, show_emails: bool) -> String {
    if label == id || !show_emails { label.to_string() } else { format!("{label} ({id})") }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max { s.to_string() } else { format!("{}…", s.chars().take(max).collect::<String>()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_ids_round_trip() {
        assert_eq!(
            Action::parse("switch:claude-code:a@b.io"),
            Action::Switch { provider: "claude-code".into(), account: "a@b.io".into() }
        );
        assert_eq!(Action::parse("save:npm"), Action::Save { provider: "npm".into() });
        assert_eq!(Action::parse("login:fly"), Action::Login { provider: "fly".into() });
        assert_eq!(Action::parse("open"), Action::Open);
        assert_eq!(Action::parse("autostart:on"), Action::Autostart { enable: false });
        assert_eq!(Action::parse("autostart:off"), Action::Autostart { enable: true });
        assert_eq!(Action::parse("noop"), Action::None);
        assert_eq!(Action::parse("switch:x"), Action::None);
    }
}
