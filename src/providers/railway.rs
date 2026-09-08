//! Railway CLI (`railway`). Live slot: `~/.railway/config.json` (whole file). Experimental:
//! key names are not documented, so the identity comes from `railway whoami`.

use super::identity::{Cmd, IdentityResolver, Parse};
use super::slot_provider::SlotProvider;
use super::slots::{FileSlot, Slot};
use super::util::text::tilde;
use super::{Provider, ProviderMeta, Tier};
use crate::core::cx::Cx;
use crate::core::model::Strategy;

fn slots(cx: &Cx) -> Vec<Box<dyn Slot>> {
    let f = cx.home().join(".railway").join("config.json");
    vec![Box::new(FileSlot::new(f.clone(), tilde(&f, cx.home())))]
}

const WHOAMI: Cmd =
    Cmd { argv: &["railway", "whoami"], parse: Parse::Regex(r"([^\s()<>]+@[^\s()<>]+)"), is_email: true, extra: &[] };

pub fn provider() -> Box<dyn Provider> {
    Box::new(SlotProvider {
        meta: ProviderMeta {
            id: "railway",
            color: "#C049FF",
            name: "Railway",
            strategy: Strategy::SlotSwap,
            tier: Tier::Experimental,
            binaries: &["railway"],
            process_names: &[],
            env_shadow: &["RAILWAY_API_TOKEN", "RAILWAY_TOKEN"],
            restart_hint: None,
            notes: "Swaps ~/.railway/config.json. The email comes from `railway whoami` (network) when a login is saved.",
            login: &["railway", "login"],
        },
        slots,
        identity: IdentityResolver::Command(WHOAMI),
        verify: Some(WHOAMI),
        extra_preflight: None,
        usage: None,
    })
}
