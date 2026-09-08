//! Turso CLI (`turso`). Live slot: `token`, `username`, `organization` in `settings.json`;
//! usage counters and update-check timestamps in the same file are preserved. Experimental
//! until confirmed on a real login.

use std::path::PathBuf;

use super::identity::IdentityResolver;
use super::slot_provider::SlotProvider;
use super::slots::{JsonKeysSlot, Slot};
use super::util::text::tilde;
use super::{Provider, ProviderMeta, Tier};
use crate::core::cx::{Cx, Os};
use crate::core::model::Strategy;

fn settings_file(cx: &Cx) -> PathBuf {
    let dir = match cx.env("TURSO_CONFIG_FOLDER") {
        Some(d) => PathBuf::from(d),
        None => match cx.os() {
            Os::Mac => cx.data_dir().join("turso"),
            Os::Linux => cx.config_dir().join("turso"),
            Os::Windows => cx.local_data_dir().join("turso"),
        },
    };
    dir.join("settings.json")
}

fn slots(cx: &Cx) -> Vec<Box<dyn Slot>> {
    let f = settings_file(cx);
    vec![Box::new(JsonKeysSlot::new(f.clone(), &["token", "username", "organization"], tilde(&f, cx.home())))]
}

pub fn provider() -> Box<dyn Provider> {
    Box::new(SlotProvider {
        meta: ProviderMeta {
            id: "turso",
            color: "#14B8A6",
            name: "Turso",
            strategy: Strategy::SlotSwap,
            tier: Tier::Experimental,
            binaries: &["turso"],
            process_names: &[],
            env_shadow: &["TURSO_API_TOKEN"],
            restart_hint: None,
            notes: "Swaps token/username/organization in Turso's settings.json (TURSO_CONFIG_FOLDER honoured).",
            login: &["turso", "auth", "login"],
        },
        slots,
        identity: IdentityResolver::JsonPointer {
            slot: 0,
            pointer: "/username",
            is_email: false,
            extra: &[("org", "/organization")],
        },
        verify: None,
        extra_preflight: None,
        usage: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn username_offline() {
        let dir = tempfile::tempdir().unwrap();
        let cx = Cx::test(dir.path(), Os::Linux);
        let f = settings_file(&cx);
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(&f, r#"{"token":"t","username":"Alice","organization":"acme","usedCommands":{"db list":3}}"#)
            .unwrap();
        let id = provider().live_identity(&cx).unwrap().unwrap();
        assert_eq!(id.id, "alice");
        assert_eq!(id.extra["org"], "acme");
    }
}
