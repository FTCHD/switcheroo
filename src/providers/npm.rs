//! npm (also pnpm, yarn v1 and bun, which read the same file). Live slot: the auth-token
//! lines for registry.npmjs.org in the user `.npmrc`; every other line is left alone.

use std::path::PathBuf;

use super::identity::{Cmd, IdentityResolver, Parse};
use super::slot_provider::SlotProvider;
use super::slots::{LinesSlot, Slot};
use super::util::text::tilde;
use super::{Provider, ProviderMeta, Tier};
use crate::core::cx::Cx;
use crate::core::model::Strategy;

fn npmrc(cx: &Cx) -> PathBuf {
    cx.env("NPM_CONFIG_USERCONFIG")
        .or_else(|| cx.env("npm_config_userconfig"))
        .map(PathBuf::from)
        .unwrap_or_else(|| cx.home().join(".npmrc"))
}

pub fn is_registry_auth_line(line: &str) -> bool {
    let l = line.trim();
    l.starts_with("//registry.npmjs.org/:_authToken=") || l.starts_with("//registry.npmjs.org/:_auth=")
}

fn slots(cx: &Cx) -> Vec<Box<dyn Slot>> {
    let f = npmrc(cx);
    vec![Box::new(LinesSlot::new(
        f.clone(),
        is_registry_auth_line,
        format!("{} (registry.npmjs.org auth lines)", tilde(&f, cx.home())),
    ))]
}

const WHOAMI: Cmd = Cmd { argv: &["npm", "whoami"], parse: Parse::Trim, is_email: false, extra: &[] };

pub fn provider() -> Box<dyn Provider> {
    Box::new(SlotProvider {
        meta: ProviderMeta {
            id: "npm",
            name: "npm",
            strategy: Strategy::SlotSwap,
            tier: Tier::Supported,
            binaries: &["npm"],
            process_names: &[],
            env_shadow: &[],
            restart_hint: None,
            notes: "Swaps only the //registry.npmjs.org/:_authToken line(s) in ~/.npmrc (or NPM_CONFIG_USERCONFIG); other settings stay. Covers pnpm, yarn v1 and bun. The username comes from `npm whoami` (network) when a login is saved.",
            login: &["npm", "login"],
        },
        slots,
        identity: IdentityResolver::Command(WHOAMI),
        verify: Some(WHOAMI),
        extra_preflight: None,
        usage: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::cx::Os;

    #[test]
    fn only_registry_auth_lines_move() {
        let dir = tempfile::tempdir().unwrap();
        let cx = Cx::test(dir.path(), Os::Linux);
        let f = dir.path().join(".npmrc");
        std::fs::write(
            &f,
            "save-exact=true\n//registry.npmjs.org/:_authToken=npm_A\n//npm.pkg.github.com/:_authToken=gh_X\n",
        )
        .unwrap();
        let p = provider();
        let cap = p.capture(&cx).unwrap().unwrap();
        std::fs::write(
            &f,
            "save-exact=true\n//registry.npmjs.org/:_authToken=npm_B\n//npm.pkg.github.com/:_authToken=gh_X\n",
        )
        .unwrap();
        p.activate(&cx, &cap.secret, &crate::core::model::Identity::new("me")).unwrap();
        assert_eq!(
            std::fs::read_to_string(&f).unwrap(),
            "save-exact=true\n//registry.npmjs.org/:_authToken=npm_A\n//npm.pkg.github.com/:_authToken=gh_X\n"
        );
    }
}
