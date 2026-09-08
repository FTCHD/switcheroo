//! Gemini CLI (`gemini`). Live slot: `~/.gemini/oauth_creds.json` (Google OAuth tokens) and
//! `~/.gemini/google_accounts.json` (`active` email). Experimental: format verified from source
//! and community switchers, not yet on a real login here. Gemini must be restarted afterwards.

use super::identity::IdentityResolver;
use super::slot_provider::SlotProvider;
use super::slots::{FileSlot, Slot};
use super::util::text::tilde;
use super::{Provider, ProviderMeta, Tier};
use crate::core::cx::Cx;
use crate::core::model::Strategy;

fn slots(cx: &Cx) -> Vec<Box<dyn Slot>> {
    let dir = cx.home().join(".gemini");
    let creds = dir.join("oauth_creds.json");
    let accounts = dir.join("google_accounts.json");
    vec![
        Box::new(FileSlot::new(creds.clone(), tilde(&creds, cx.home()))),
        Box::new(FileSlot::new(accounts.clone(), tilde(&accounts, cx.home()))),
    ]
}

pub fn provider() -> Box<dyn Provider> {
    Box::new(SlotProvider {
        meta: ProviderMeta {
            id: "gemini-cli",
            name: "Gemini CLI",
            strategy: Strategy::SlotSwap,
            tier: Tier::Experimental,
            binaries: &["gemini"],
            process_names: &["gemini"],
            env_shadow: &["GEMINI_API_KEY", "GOOGLE_APPLICATION_CREDENTIALS"],
            restart_hint: Some("Gemini CLI reads credentials at startup; quit and restart it."),
            notes: "Swaps ~/.gemini/oauth_creds.json and google_accounts.json together. Log in by running `gemini` and using /auth.",
            login: &["gemini"],
        },
        slots,
        identity: IdentityResolver::JsonPointer { slot: 1, pointer: "/active", is_email: true, extra: &[] },
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
    fn active_email_from_google_accounts() {
        let dir = tempfile::tempdir().unwrap();
        let cx = Cx::test(dir.path(), Os::Linux);
        std::fs::create_dir_all(dir.path().join(".gemini")).unwrap();
        std::fs::write(dir.path().join(".gemini/oauth_creds.json"), r#"{"access_token":"a","refresh_token":"r"}"#)
            .unwrap();
        std::fs::write(dir.path().join(".gemini/google_accounts.json"), r#"{"active":"g@gmail.com","old":[]}"#)
            .unwrap();
        assert_eq!(provider().live_identity(&cx).unwrap().unwrap().id, "g@gmail.com");
    }
}
