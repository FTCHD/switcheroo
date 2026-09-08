//! Fly.io (`flyctl`). Live slot: `~/.fly/config.yml` (whole file; it also holds per-account
//! WireGuard state). Writes take flyctl's own lock file first.

use super::identity::{Cmd, IdentityResolver, Parse};
use super::slot_provider::SlotProvider;
use super::slots::{FileSlot, LockedSlot, Slot};
use super::util::text::tilde;
use super::{Provider, ProviderMeta, Tier};
use crate::core::cx::Cx;
use crate::core::model::Strategy;

fn slots(cx: &Cx) -> Vec<Box<dyn Slot>> {
    let dir = cx.home().join(".fly");
    let f = dir.join("config.yml");
    vec![Box::new(LockedSlot::new(
        Box::new(FileSlot::new(f.clone(), tilde(&f, cx.home()))),
        dir.join("flyctl.config.lock"),
    ))]
}

const WHOAMI: Cmd =
    Cmd { argv: &["flyctl", "auth", "whoami", "--json"], parse: Parse::Json("/email"), is_email: true, extra: &[] };

pub fn provider() -> Box<dyn Provider> {
    Box::new(SlotProvider {
        meta: ProviderMeta {
            id: "fly",
            name: "Fly.io",
            strategy: Strategy::SlotSwap,
            tier: Tier::Supported,
            binaries: &["flyctl", "fly"],
            process_names: &["flyctl"],
            env_shadow: &["FLY_API_TOKEN", "FLY_ACCESS_TOKEN"],
            restart_hint: None,
            notes: "Swaps ~/.fly/config.yml (taking flyctl's lock file first). The email comes from `fly auth whoami --json` (network) when a login is saved.",
            login: &["flyctl", "auth", "login"],
        },
        slots,
        identity: IdentityResolver::Command(WHOAMI),
        verify: Some(WHOAMI),
        extra_preflight: None,
    })
}
