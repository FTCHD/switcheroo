//! The operations: status, save, use (switch), remove, rename, login, doctor. Every mutation
//! runs under the cross-process lock and re-captures the live login before replacing it.

use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use chrono::Utc;
use serde::Serialize;

use super::lock::{self, LockGuard};
use super::model::*;
use super::state::LiveCache;
use super::terminal;
use super::{Core, Strategy};
use crate::providers::Provider;

/// `use`/`save` can fail for reasons the user must fix; these map to distinct exit codes.
#[derive(Debug, thiserror::Error)]
pub enum OpError {
    #[error("{0} is not installed (looked for {1} on PATH)")]
    NotInstalled(String, String),
    #[error("{0} has no login to save")]
    NothingLoggedIn(String),
}

#[derive(Debug, Serialize)]
pub struct Doctor {
    pub version: String,
    pub data_dir: String,
    pub os: String,
    pub vault: String,
    pub vault_ok: bool,
    pub vault_error: Option<String>,
    pub path: Vec<String>,
    pub providers: Vec<ProviderStatus>,
}

impl Core {
    fn lock(&self) -> Result<LockGuard> {
        lock::acquire(&self.dirs.lock_file(), Duration::from_secs(15))
    }

    fn require_installed(&self, p: &dyn Provider) -> Result<Installed> {
        self.installed(p)
            .ok_or_else(|| OpError::NotInstalled(p.meta().name.to_string(), p.meta().binaries.join("/")).into())
    }

    /// Live identity with the fingerprint cache, so network `whoami` runs only on change.
    pub fn live_identity(&self, p: &dyn Provider, refresh: bool) -> Result<Option<Identity>> {
        let id = p.meta().id;
        let fp = p.fingerprint(&self.cx)?;
        if !refresh
            && let Some(fp) = fp
            && let Some(cached) = self.state().live_cache.get(id)
            && cached.fingerprint == fp
        {
            return Ok(cached.identity.clone());
        }
        let live = p.live_identity(&self.cx)?;
        if let Some(fp) = fp {
            let mut st = self.state();
            st.live_cache.insert(id.to_string(), LiveCache { fingerprint: fp, identity: live.clone(), at: Utc::now() });
            self.persist(&mut st, "state.changed", Some(id), None)?;
        }
        Ok(live)
    }

    pub fn status(&self, p: &dyn Provider, refresh: bool) -> ProviderStatus {
        let meta = p.meta();
        let installed = self.installed(p);
        let mut warnings = Vec::new();
        let mut live = None;
        let mut live_checked_at = None;
        let mut accounts = self.state().accounts_for(meta.id);
        if installed.is_some() && !matches!(meta.tier, crate::providers::Tier::Unsupported(_)) {
            match self.live_identity(p, refresh) {
                Ok(l) => {
                    live = l;
                    live_checked_at = self.state().live_cache.get(meta.id).map(|c| c.at).or(Some(Utc::now()));
                }
                Err(e) => {
                    warnings.push(Warning::warn("live-identity", format!("could not read the current login: {e:#}")))
                }
            }
            if meta.strategy == Strategy::NativeSwitch {
                match p.native_list(&self.cx) {
                    Ok(list) => {
                        for ident in list {
                            if !accounts.iter().any(|a| a.id == ident.id) {
                                accounts.push(Account {
                                    provider: meta.id.to_string(),
                                    id: ident.id.clone(),
                                    label: ident.label.clone(),
                                    identity: ident,
                                    saved_at: Utc::now(),
                                    last_used: None,
                                    has_secret: false,
                                });
                            }
                        }
                    }
                    Err(e) => warnings.push(Warning::warn("native-list", format!("{e:#}"))),
                }
            }
            warnings.extend(p.preflight(&self.cx));
        }
        let active_account = live.as_ref().and_then(|l| accounts.iter().find(|a| a.id == l.id).map(|a| a.id.clone()));
        ProviderStatus {
            info: meta.info(),
            installed,
            live,
            active_account,
            accounts,
            warnings,
            slots: p.slot_descriptions(&self.cx),
            live_checked_at,
        }
    }

    pub fn status_all(&self, refresh: bool) -> Vec<ProviderStatus> {
        self.providers.iter().map(|p| self.status(p.as_ref(), refresh)).collect()
    }

    /// Capture the live login into the vault (or record the active native account).
    pub fn save(&self, provider_id: &str, label: Option<String>) -> Result<Account> {
        let p = self.provider(provider_id)?;
        let _lock = self.lock()?;
        self.require_installed(p)?;
        let meta = p.meta();
        match meta.strategy {
            Strategy::NativeSwitch => {
                let live = p.live_identity(&self.cx)?.ok_or_else(|| OpError::NothingLoggedIn(meta.name.to_string()))?;
                let mut st = self.state();
                let acct = st.upsert(
                    Account {
                        provider: meta.id.to_string(),
                        id: live.id.clone(),
                        label: live.label.clone(),
                        identity: live,
                        saved_at: Utc::now(),
                        last_used: None,
                        has_secret: false,
                    },
                    label,
                );
                self.persist(&mut st, "account.saved", Some(meta.id), Some(&acct.id))?;
                Ok(acct)
            }
            Strategy::SlotSwap => {
                let captured = p.capture(&self.cx)?.ok_or_else(|| OpError::NothingLoggedIn(meta.name.to_string()))?;
                let identity = match (captured.identity, &label) {
                    (Some(i), _) => i,
                    (None, Some(l)) => Identity::new(l.clone()),
                    (None, None) => match captured.identity_error {
                        Some(e) => bail!(
                            "could not determine who is logged in to {} ({}); run `switcheroo save {} --label <name>` to save it anyway",
                            meta.name,
                            e,
                            meta.id
                        ),
                        None => bail!(
                            "{} does not record who is logged in; run `switcheroo save {} --label <name>`",
                            meta.name,
                            meta.id
                        ),
                    },
                };
                self.store_account(p, identity, &captured.secret, label)
            }
        }
    }

    fn store_account(
        &self,
        p: &dyn Provider,
        identity: Identity,
        secret: &SecretBlob,
        label: Option<String>,
    ) -> Result<Account> {
        let meta = p.meta();
        let key = vault_key(meta.id, &identity.id);
        let vault_label = label.clone().unwrap_or_else(|| identity.label.clone());
        self.vault.put(&key, &format!("{} {}", meta.name, vault_label), secret).context("storing the credential")?;
        let mut st = self.state();
        let acct = st.upsert(
            Account {
                provider: meta.id.to_string(),
                id: identity.id.clone(),
                label: identity.label.clone(),
                identity: identity.clone(),
                saved_at: Utc::now(),
                last_used: None,
                has_secret: true,
            },
            label,
        );
        if let Ok(Some(fp)) = p.fingerprint(&self.cx) {
            st.live_cache
                .insert(meta.id.to_string(), LiveCache { fingerprint: fp, identity: Some(identity), at: Utc::now() });
        }
        self.persist(&mut st, "account.saved", Some(meta.id), Some(&acct.id))?;
        Ok(acct)
    }

    /// Find a saved (or native) account by id, email, label, or unique prefix/substring.
    pub fn resolve_account(&self, p: &dyn Provider, query: &str) -> Result<Account> {
        let meta = p.meta();
        let mut candidates = self.state().accounts_for(meta.id);
        if meta.strategy == Strategy::NativeSwitch
            && let Ok(list) = p.native_list(&self.cx)
        {
            for ident in list {
                if !candidates.iter().any(|a| a.id == ident.id) {
                    candidates.push(Account {
                        provider: meta.id.to_string(),
                        id: ident.id.clone(),
                        label: ident.label.clone(),
                        identity: ident,
                        saved_at: Utc::now(),
                        last_used: None,
                        has_secret: false,
                    });
                }
            }
        }
        if candidates.is_empty() {
            bail!(
                "no saved accounts for {}; run `switcheroo save {}` or `switcheroo login {}` first",
                meta.name,
                meta.id,
                meta.id
            );
        }
        let q = normalize_id(query);
        if let Some(a) = candidates.iter().find(|a| {
            a.id == q || a.label.to_lowercase() == q || a.identity.email.as_deref().map(normalize_id) == Some(q.clone())
        }) {
            return Ok(a.clone());
        }
        let partial: Vec<&Account> =
            candidates.iter().filter(|a| a.id.contains(&q) || a.label.to_lowercase().contains(&q)).collect();
        match partial.len() {
            1 => Ok(partial[0].clone()),
            0 => bail!(
                "no account matching '{}' for {}. Saved: {}",
                query,
                meta.name,
                candidates.iter().map(|a| a.id.as_str()).collect::<Vec<_>>().join(", ")
            ),
            _ => bail!(
                "'{}' is ambiguous for {}: {}",
                query,
                meta.name,
                partial.iter().map(|a| a.id.as_str()).collect::<Vec<_>>().join(", ")
            ),
        }
    }

    /// Switch the live login. Re-captures whatever is live first, then activates, then verifies.
    pub fn use_account(&self, provider_id: &str, query: &str) -> Result<SwitchOutcome> {
        let p = self.provider(provider_id)?;
        let _lock = self.lock()?;
        self.require_installed(p)?;
        let meta = p.meta();
        let target = self.resolve_account(p, query)?;
        let mut warnings = p.preflight(&self.cx);
        let mut recaptured: Option<Identity> = None;

        match meta.strategy {
            Strategy::NativeSwitch => {
                p.native_switch(&self.cx, &target.id)
                    .with_context(|| format!("switching {} to {}", meta.name, target.label))?;
                match p.verify(&self.cx, &target.identity) {
                    Ok(true) => {}
                    Ok(false) => bail!("{} did not report {} as active after the switch", meta.name, target.label),
                    Err(e) => warnings.push(Warning::warn("unverified", format!("could not verify the switch: {e:#}"))),
                }
            }
            Strategy::SlotSwap => {
                let secret = self.vault.get(&target.vault_key())?.ok_or_else(|| {
                    anyhow!("no saved credential for {} ({}); save it again", target.label, meta.name)
                })?;
                let previous = p.capture(&self.cx).context("reading the current login")?;
                if let Some(prev) = &previous {
                    match &prev.identity {
                        Some(ident) => {
                            let key = vault_key(meta.id, &ident.id);
                            self.vault
                                .put(&key, &format!("{} {}", meta.name, ident.label), &prev.secret)
                                .context("re-saving the current login")?;
                            if ident.id != target.id {
                                recaptured = Some(ident.clone());
                            }
                        }
                        None => warnings.push(Warning::info(
                            "recapture-skipped",
                            match &prev.identity_error {
                                Some(e) => format!("the current login could not be identified ({e}), so it was not saved before switching"),
                                None => "the current login could not be identified, so it was not saved before switching".to_string(),
                            },
                        )),
                    }
                }
                p.activate(&self.cx, &secret, &target.identity)
                    .with_context(|| format!("writing {} credentials", meta.name))?;
                match p.verify(&self.cx, &target.identity) {
                    Ok(true) => {}
                    Ok(false) => {
                        let restored = match &previous {
                            Some(prev) => {
                                p.activate(&self.cx, &prev.secret, prev.identity.as_ref().unwrap_or(&target.identity))
                            }
                            None => p.clear(&self.cx),
                        };
                        let note = match restored {
                            Ok(()) => "restored the previous login",
                            Err(_) => "and the previous login could not be restored",
                        };
                        bail!("{} did not report {} as logged in after the switch; {}", meta.name, target.label, note);
                    }
                    Err(e) => warnings.push(Warning::warn("unverified", format!("could not verify the switch: {e:#}"))),
                }
            }
        }

        let mut st = self.state();
        if let Some(prev) = &recaptured {
            st.upsert(
                Account {
                    provider: meta.id.to_string(),
                    id: prev.id.clone(),
                    label: prev.label.clone(),
                    identity: prev.clone(),
                    saved_at: Utc::now(),
                    last_used: None,
                    has_secret: true,
                },
                None,
            );
        }
        let account = match st.find_mut(meta.id, &target.id) {
            Some(a) => {
                a.last_used = Some(Utc::now());
                if meta.strategy == Strategy::SlotSwap {
                    a.saved_at = Utc::now();
                }
                a.clone()
            }
            None => {
                let mut a = target.clone();
                a.last_used = Some(Utc::now());
                st.upsert(a, None)
            }
        };
        if let Ok(Some(fp)) = p.fingerprint(&self.cx) {
            st.live_cache.insert(
                meta.id.to_string(),
                LiveCache { fingerprint: fp, identity: Some(target.identity.clone()), at: Utc::now() },
            );
        } else {
            st.live_cache.remove(meta.id);
        }
        self.persist(&mut st, "switch.done", Some(meta.id), Some(&account.id))?;
        if let Some(hint) = meta.restart_hint {
            warnings.push(Warning::info("restart", hint));
        }
        Ok(SwitchOutcome { provider: meta.id.to_string(), account, recaptured: recaptured.map(|i| i.id), warnings })
    }

    pub fn remove(&self, provider_id: &str, query: &str) -> Result<Account> {
        let p = self.provider(provider_id)?;
        let _lock = self.lock()?;
        let meta = p.meta();
        let acct = self.resolve_account(p, query)?;
        if acct.has_secret {
            self.vault.delete(&acct.vault_key()).context("removing the stored credential")?;
        }
        let mut st = self.state();
        st.remove(meta.id, &acct.id);
        self.persist(&mut st, "account.removed", Some(meta.id), Some(&acct.id))?;
        Ok(acct)
    }

    pub fn rename(&self, provider_id: &str, query: &str, label: &str) -> Result<Account> {
        let p = self.provider(provider_id)?;
        let _lock = self.lock()?;
        let meta = p.meta();
        let acct = self.resolve_account(p, query)?;
        let mut st = self.state();
        let updated = st.upsert(acct, Some(label.trim().to_string()));
        self.persist(&mut st, "account.renamed", Some(meta.id), Some(&updated.id))?;
        Ok(updated)
    }

    /// The command a terminal should run to add an account for `provider_id`.
    pub fn login_command_line(&self, provider_id: &str) -> Result<String> {
        let p = self.provider(provider_id)?;
        let exe = std::env::current_exe().context("current executable")?;
        Ok(format!("{} login {}", shell_quote(&exe.to_string_lossy()), p.meta().id))
    }

    /// Run the CLI's login in this terminal, then save the result.
    pub fn login_here(&self, provider_id: &str, label: Option<String>) -> Result<Account> {
        let p = self.provider(provider_id)?;
        self.require_installed(p)?;
        let argv = p.login_command(&self.cx);
        if argv.is_empty() {
            bail!(
                "{} has no scripted login; log in with the CLI, then run `switcheroo save {}`",
                p.meta().name,
                p.meta().id
            );
        }
        let code = self.cx.run_interactive(&argv).with_context(|| format!("running {}", argv.join(" ")))?;
        if code != 0 {
            bail!("`{}` exited with status {}", argv.join(" "), code);
        }
        self.save(provider_id, label)
    }

    /// Open a terminal window running `switcheroo login <provider>`.
    pub fn login_spawn(&self, provider_id: &str) -> Result<(bool, String)> {
        let cmd = self.login_command_line(provider_id)?;
        let spawned = terminal::open_terminal(&cmd)?;
        Ok((spawned, cmd))
    }

    pub fn doctor(&self) -> Doctor {
        let (vault_ok, vault_error) = match self.vault.health() {
            Ok(_) => (true, None),
            Err(e) => (false, Some(format!("{e:#}"))),
        };
        Doctor {
            version: env!("CARGO_PKG_VERSION").to_string(),
            data_dir: self.dirs.data.display().to_string(),
            os: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
            vault: self.vault.name().to_string(),
            vault_ok,
            vault_error,
            path: self.cx.path_string().split(if cfg!(windows) { ';' } else { ':' }).map(str::to_string).collect(),
            providers: self.status_all(true),
        }
    }
}

fn shell_quote(s: &str) -> String {
    if s.chars().all(|c| c.is_ascii_alphanumeric() || "/._-:\\".contains(c)) {
        s.to_string()
    } else if cfg!(windows) {
        format!("\"{}\"", s)
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::cx::Os;
    use crate::core::settings::VaultChoice;
    use crate::core::{CoreOpts, Cx};
    use crate::providers::slot_provider::testing::json_file_provider;

    fn core(dir: &std::path::Path) -> std::sync::Arc<Core> {
        let cx = Cx::test(dir, Os::Linux);
        Core::open(CoreOpts {
            data_dir: Some(dir.join("data")),
            vault: Some(VaultChoice::File),
            cx: Some(cx),
            providers: Some(vec![Box::new(json_file_provider())]),
        })
        .unwrap()
    }

    #[test]
    fn save_then_switch_recaptures_live_login_first() {
        let dir = tempfile::tempdir().unwrap();
        let core = core(dir.path());
        let file = dir.path().join("mock.json");
        let p = core.provider("mock").unwrap();
        assert!(core.installed(p).is_none()); // no binary → status still works, save refuses
        assert!(core.save("mock", None).is_err());

        // Pretend it is installed for the rest of the test.
        core.detect_cache.lock().unwrap().insert("mock", Some(Installed { path: "/bin/mock".into(), version: None }));

        std::fs::write(&file, r#"{"token":"t-a","user":"a@x.io","theme":"dark"}"#).unwrap();
        let a = core.save("mock", Some("Work".into())).unwrap();
        assert_eq!(a.id, "a@x.io");
        assert_eq!(a.label, "Work");

        // User logs in as B with the CLI; Switcheroo has never seen B.
        std::fs::write(&file, r#"{"token":"t-b","user":"b@x.io","theme":"dark"}"#).unwrap();
        let st = core.status(p, false);
        assert_eq!(st.live.as_ref().unwrap().id, "b@x.io");
        assert!(st.active_account.is_none());

        let out = core.use_account("mock", "work").unwrap();
        assert_eq!(out.account.id, "a@x.io");
        assert_eq!(out.recaptured.as_deref(), Some("b@x.io"));
        let v: serde_json::Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
        assert_eq!(v["token"], "t-a");
        assert_eq!(v["theme"], "dark");

        // B was auto-registered with a fresh secret and can be switched back to.
        let accounts = core.state().accounts_for("mock");
        assert_eq!(accounts.len(), 2);
        let out = core.use_account("mock", "b@x").unwrap();
        assert_eq!(out.account.id, "b@x.io");
        let v: serde_json::Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
        assert_eq!(v["token"], "t-b");

        // status reports the active account without re-resolving.
        let st = core.status(p, false);
        assert_eq!(st.active_account.as_deref(), Some("b@x.io"));

        core.remove("mock", "Work").unwrap();
        assert_eq!(core.state().accounts_for("mock").len(), 1);
        assert!(core.vault.get("mock:a@x.io").unwrap().is_none());
    }

    #[test]
    fn ambiguous_and_missing_queries_fail_clearly() {
        let dir = tempfile::tempdir().unwrap();
        let core = core(dir.path());
        core.detect_cache.lock().unwrap().insert("mock", Some(Installed { path: "/bin/mock".into(), version: None }));
        let file = dir.path().join("mock.json");
        std::fs::write(&file, r#"{"token":"1","user":"one@x.io"}"#).unwrap();
        core.save("mock", None).unwrap();
        std::fs::write(&file, r#"{"token":"2","user":"two@x.io"}"#).unwrap();
        core.save("mock", None).unwrap();
        let p = core.provider("mock").unwrap();
        assert!(core.resolve_account(p, "x.io").unwrap_err().to_string().contains("ambiguous"));
        assert!(core.resolve_account(p, "nobody").unwrap_err().to_string().contains("no account matching"));
        assert_eq!(core.resolve_account(p, "TWO@X.IO").unwrap().id, "two@x.io");
        core.rename("mock", "one", "Personal").unwrap();
        assert_eq!(core.resolve_account(p, "personal").unwrap().id, "one@x.io");
    }
}
