//! Usage/quota lookups with a short in-process cache, so the web UI, tray and CLI can ask
//! freely without hammering the providers' endpoints. `status()` never touches this.

use std::time::{Duration, Instant};

use anyhow::Result;

use super::Core;
use super::model::Usage;
use crate::providers::Provider;

pub const USAGE_TTL: Duration = Duration::from_secs(60);

impl Core {
    /// Usage for the live login of `p`, or `None` when the provider has none / nobody is signed in.
    pub fn usage(&self, p: &dyn Provider, refresh: bool) -> Result<Option<Usage>> {
        let id = p.meta().id;
        if !p.supports_usage() || self.installed(p).is_none() {
            return Ok(None);
        }
        if self.live_identity(p, false)?.is_none() {
            self.usage_cache.lock().unwrap().remove(id);
            return Ok(None);
        }
        if !refresh
            && let Some((at, cached)) = self.usage_cache.lock().unwrap().get(id)
            && at.elapsed() < USAGE_TTL
        {
            return Ok(Some(cached.clone()));
        }
        let fresh = p.usage(&self.cx)?;
        let mut cache = self.usage_cache.lock().unwrap();
        match &fresh {
            Some(u) => {
                cache.insert(id, (Instant::now(), u.clone()));
            }
            None => {
                cache.remove(id);
            }
        }
        Ok(fresh)
    }

    /// Forget cached usage after the live login changed.
    pub fn invalidate_usage(&self, id: &str) {
        self.usage_cache.lock().unwrap().remove(id);
    }
}
