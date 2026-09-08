//! `state.json`: saved accounts, labels, timestamps, and the cached live identity per
//! provider. No secrets. Written atomically; `revision` increases on every save so other
//! processes (tray, server) can tell when to reload.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::fsutil::{read_opt, write_atomic};
use super::model::{Account, Identity};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LiveCache {
    pub fingerprint: u64,
    pub identity: Option<Identity>,
    pub at: DateTime<Utc>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct State {
    #[serde(default)]
    pub revision: u64,
    #[serde(default)]
    pub accounts: Vec<Account>,
    #[serde(default)]
    pub live_cache: BTreeMap<String, LiveCache>,
}

impl State {
    pub fn load(path: &Path) -> Result<State> {
        match read_opt(path)? {
            Some(bytes) => serde_json::from_slice(&bytes).with_context(|| format!("parsing {}", path.display())),
            None => Ok(State::default()),
        }
    }

    pub fn save(&mut self, path: &Path) -> Result<()> {
        self.revision += 1;
        let bytes = serde_json::to_vec_pretty(self)?;
        write_atomic(path, &bytes, 0o600)
    }

    pub fn accounts_for(&self, provider: &str) -> Vec<Account> {
        self.accounts.iter().filter(|a| a.provider == provider).cloned().collect()
    }

    pub fn find_mut(&mut self, provider: &str, id: &str) -> Option<&mut Account> {
        self.accounts.iter_mut().find(|a| a.provider == provider && a.id == id)
    }

    /// Insert or update, keeping the existing label unless a new one is given.
    pub fn upsert(&mut self, mut account: Account, label: Option<String>) -> Account {
        if let Some(existing) = self.find_mut(&account.provider, &account.id) {
            if let Some(l) = label {
                existing.label = l;
            }
            existing.identity = account.identity;
            existing.saved_at = account.saved_at;
            existing.has_secret = account.has_secret || existing.has_secret;
            return existing.clone();
        }
        if let Some(l) = label {
            account.label = l;
        }
        self.accounts.push(account.clone());
        account
    }

    pub fn remove(&mut self, provider: &str, id: &str) -> Option<Account> {
        let idx = self.accounts.iter().position(|a| a.provider == provider && a.id == id)?;
        Some(self.accounts.remove(idx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn acct(id: &str) -> Account {
        Account {
            provider: "p".into(),
            id: id.into(),
            label: id.into(),
            identity: Identity::new(id),
            saved_at: Utc::now(),
            last_used: None,
            has_secret: true,
        }
    }

    #[test]
    fn round_trip_and_revision() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("state.json");
        let mut s = State::load(&p).unwrap();
        assert_eq!(s.revision, 0);
        s.upsert(acct("a"), Some("Work".into()));
        s.save(&p).unwrap();
        let s2 = State::load(&p).unwrap();
        assert_eq!(s2.revision, 1);
        assert_eq!(s2.accounts[0].label, "Work");
    }

    #[test]
    fn upsert_keeps_label_when_none_given() {
        let mut s = State::default();
        s.upsert(acct("a"), Some("Work".into()));
        s.upsert(acct("a"), None);
        assert_eq!(s.accounts.len(), 1);
        assert_eq!(s.accounts[0].label, "Work");
        assert!(s.remove("p", "a").is_some());
        assert!(s.accounts.is_empty());
    }
}
