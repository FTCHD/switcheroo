//! `Core` is the one object every surface talks to: providers, vault, persisted state,
//! settings, and the event bus. CLI, HTTP API and tray never reach around it.

pub mod appdirs;
pub mod bus;
pub mod cx;
pub mod fsutil;
pub mod lock;
pub mod model;
pub mod proc;
pub mod settings;
pub mod state;
pub mod switch;
pub mod terminal;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, RwLock};

use anyhow::{Context, Result, anyhow};

pub use cx::Cx;
pub use model::*;

use appdirs::AppDirs;
use bus::{Bus, Event};
use settings::{Settings, VaultChoice};
use state::State;

use crate::providers::{self, Provider};
use crate::vault::{self, Vault};

pub struct Core {
    pub dirs: AppDirs,
    pub cx: Cx,
    pub settings: RwLock<Settings>,
    state: Mutex<State>,
    pub vault: Box<dyn Vault>,
    pub providers: Vec<Box<dyn Provider>>,
    pub bus: Bus,
    detect_cache: Mutex<HashMap<&'static str, Option<Installed>>>,
}

#[derive(Default)]
pub struct CoreOpts {
    pub data_dir: Option<PathBuf>,
    pub vault: Option<VaultChoice>,
    pub cx: Option<Cx>,
    pub providers: Option<Vec<Box<dyn Provider>>>,
}

impl Core {
    pub fn open(opts: CoreOpts) -> Result<Arc<Core>> {
        let dirs = AppDirs::resolve(opts.data_dir)?;
        let settings = Settings::load(&dirs.settings_file())?;
        let vault_choice = opts.vault.unwrap_or(settings.vault);
        let vault = vault::open(vault_choice, &dirs)?;
        let cx = match opts.cx {
            Some(c) => c,
            None => Cx::from_process()?,
        };
        let state = State::load(&dirs.state_file())?;
        let providers = opts.providers.unwrap_or_else(providers::all);
        Ok(Arc::new(Core {
            dirs,
            cx,
            settings: RwLock::new(settings),
            state: Mutex::new(state),
            vault,
            providers,
            bus: Bus::new(),
            detect_cache: Mutex::new(HashMap::new()),
        }))
    }

    /// Look a provider up by id (exact) or name (case-insensitive).
    pub fn provider(&self, id: &str) -> Result<&dyn Provider> {
        let wanted = id.trim().to_lowercase();
        self.providers
            .iter()
            .find(|p| p.meta().id == wanted || p.meta().name.to_lowercase() == wanted)
            .map(|p| p.as_ref())
            .ok_or_else(|| {
                anyhow!(
                    "unknown provider '{}'. Known providers: {}",
                    id,
                    self.providers.iter().map(|p| p.meta().id).collect::<Vec<_>>().join(", ")
                )
            })
    }

    pub fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn revision(&self) -> u64 {
        self.state().revision
    }

    /// Re-read `state.json` (another process may have written it). Returns true when changed.
    pub fn reload_state(&self) -> Result<bool> {
        let fresh = State::load(&self.dirs.state_file())?;
        let mut st = self.state();
        if fresh.revision != st.revision {
            *st = fresh;
            let revision = st.revision;
            drop(st);
            self.bus.publish(Event {
                kind: "state.changed".into(),
                provider: None,
                account: None,
                message: None,
                revision,
            });
            return Ok(true);
        }
        Ok(false)
    }

    pub fn save_settings(&self, settings: Settings) -> Result<()> {
        settings.save(&self.dirs.settings_file()).context("saving settings")?;
        *self.settings.write().unwrap_or_else(|e| e.into_inner()) = settings;
        self.bus.publish(Event {
            kind: "settings.changed".into(),
            provider: None,
            account: None,
            message: None,
            revision: self.revision(),
        });
        Ok(())
    }

    /// Cached per process: `--version` calls are slow for some CLIs.
    pub fn installed(&self, p: &dyn Provider) -> Option<Installed> {
        let id = p.meta().id;
        if let Some(hit) = self.detect_cache.lock().unwrap().get(id) {
            return hit.clone();
        }
        let found = p.detect(&self.cx);
        self.detect_cache.lock().unwrap().insert(id, found.clone());
        found
    }

    pub(crate) fn persist(
        &self,
        st: &mut State,
        kind: &str,
        provider: Option<&str>,
        account: Option<&str>,
    ) -> Result<()> {
        st.save(&self.dirs.state_file()).context("saving state")?;
        self.bus.publish(Event {
            kind: kind.to_string(),
            provider: provider.map(str::to_string),
            account: account.map(str::to_string),
            message: None,
            revision: st.revision,
        });
        Ok(())
    }
}
