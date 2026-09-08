//! Expo / EAS (`eas`, `expo`). Live slot: the `auth` object of `~/.expo/state.json`; the
//! `uuid` device id in the same file is left alone. Experimental: the `auth` sub-keys are
//! documented by the community, not by Expo.

use super::identity::{Cmd, IdentityResolver, Parse};
use super::slot_provider::SlotProvider;
use super::slots::{JsonKeysSlot, Slot};
use super::util::text::tilde;
use super::{Provider, ProviderMeta, Tier};
use crate::core::cx::Cx;
use crate::core::model::Strategy;

fn slots(cx: &Cx) -> Vec<Box<dyn Slot>> {
    let f = cx.home().join(".expo").join("state.json");
    vec![Box::new(JsonKeysSlot::new(f.clone(), &["auth"], tilde(&f, cx.home())))]
}

pub fn provider() -> Box<dyn Provider> {
    Box::new(SlotProvider {
        meta: ProviderMeta {
            id: "expo",
            color: "#4630EB",
            name: "Expo / EAS",
            strategy: Strategy::SlotSwap,
            tier: Tier::Experimental,
            binaries: &["eas", "expo"],
            process_names: &[],
            env_shadow: &["EXPO_TOKEN"],
            restart_hint: None,
            notes: "Swaps the auth object in ~/.expo/state.json (shared by Expo CLI and EAS CLI).",
            login: &["eas", "login"],
        },
        slots,
        identity: IdentityResolver::Chain(&[
            IdentityResolver::JsonPointer { slot: 0, pointer: "/auth/username", is_email: false, extra: &[] },
            IdentityResolver::Command(Cmd {
                argv: &["eas", "whoami"],
                parse: Parse::Trim,
                is_email: false,
                extra: &[],
            }),
        ]),
        verify: None,
        extra_preflight: None,
        usage: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::cx::Os;

    #[test]
    fn username_from_auth_object_and_uuid_preserved() {
        let dir = tempfile::tempdir().unwrap();
        let cx = Cx::test(dir.path(), Os::Linux);
        std::fs::create_dir_all(dir.path().join(".expo")).unwrap();
        let f = dir.path().join(".expo/state.json");
        std::fs::write(&f, r#"{"uuid":"dev-1","auth":{"sessionSecret":"s","userId":"u","username":"alice"}}"#).unwrap();
        let p = provider();
        assert_eq!(p.live_identity(&cx).unwrap().unwrap().id, "alice");
        let cap = p.capture(&cx).unwrap().unwrap();
        std::fs::write(&f, r#"{"uuid":"dev-1"}"#).unwrap();
        assert!(p.capture(&cx).unwrap().is_none(), "logged-out state has no auth key");
        p.activate(&cx, &cap.secret, cap.identity.as_ref().unwrap()).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&std::fs::read(&f).unwrap()).unwrap();
        assert_eq!(v["uuid"], "dev-1");
        assert_eq!(v["auth"]["username"], "alice");
    }
}
