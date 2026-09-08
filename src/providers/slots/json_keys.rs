//! Only some top-level keys of a JSON document are the credential slot; everything else in the
//! file (preferences, project state, device ids) is preserved.

use std::path::PathBuf;

use anyhow::{Context, Result};
use serde_json::{Map, Value};

use super::Slot;
use crate::core::fsutil::{read_opt, write_atomic};

pub struct JsonKeysSlot {
    pub path: PathBuf,
    pub keys: Vec<&'static str>,
    pub display: String,
}

impl JsonKeysSlot {
    pub fn new(path: PathBuf, keys: &[&'static str], display: String) -> JsonKeysSlot {
        JsonKeysSlot { path, keys: keys.to_vec(), display }
    }

    fn load(&self) -> Result<Option<Map<String, Value>>> {
        let Some(bytes) = read_opt(&self.path)? else { return Ok(None) };
        let value: Value =
            serde_json::from_slice(&bytes).with_context(|| format!("parsing {}", self.path.display()))?;
        match value {
            Value::Object(m) => Ok(Some(m)),
            _ => anyhow::bail!("{} is not a JSON object", self.path.display()),
        }
    }

    fn store(&self, map: &Map<String, Value>) -> Result<()> {
        write_atomic(&self.path, &serde_json::to_vec_pretty(map)?, 0o600)
    }
}

impl Slot for JsonKeysSlot {
    fn read(&self) -> Result<Option<Vec<u8>>> {
        let Some(map) = self.load()? else { return Ok(None) };
        let mut picked = Map::new();
        for k in &self.keys {
            if let Some(v) = map.get(*k) {
                picked.insert(k.to_string(), v.clone());
            }
        }
        if picked.is_empty() {
            return Ok(None);
        }
        Ok(Some(serde_json::to_vec(&picked)?))
    }

    fn write(&self, data: &[u8]) -> Result<()> {
        let incoming: Map<String, Value> = serde_json::from_slice(data).context("slot data is not a JSON object")?;
        let mut map = self.load()?.unwrap_or_default();
        for k in &self.keys {
            match incoming.get(*k) {
                Some(v) => {
                    map.insert(k.to_string(), v.clone());
                }
                None => {
                    map.remove(*k);
                }
            }
        }
        self.store(&map)
    }

    fn clear(&self) -> Result<()> {
        let Some(mut map) = self.load()? else { return Ok(()) };
        for k in &self.keys {
            map.remove(*k);
        }
        self.store(&map)
    }

    fn describe(&self) -> String {
        format!("{} → {}", self.display, self.keys.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_and_merges_only_named_keys() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("cfg.json");
        std::fs::write(&p, r#"{"keep":1,"token":"old","other":{"x":true}}"#).unwrap();
        let slot = JsonKeysSlot::new(p.clone(), &["token", "team"], "cfg".into());
        let read = slot.read().unwrap().unwrap();
        assert_eq!(serde_json::from_slice::<Value>(&read).unwrap(), serde_json::json!({"token":"old"}));

        slot.write(br#"{"token":"new","team":"t1"}"#).unwrap();
        let after: Value = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
        assert_eq!(after["keep"], 1);
        assert_eq!(after["other"]["x"], true);
        assert_eq!(after["token"], "new");
        assert_eq!(after["team"], "t1");

        slot.write(br#"{"token":"x"}"#).unwrap(); // team absent → removed
        let after: Value = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
        assert!(after.get("team").is_none());

        slot.clear().unwrap();
        let after: Value = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
        assert!(after.get("token").is_none());
        assert_eq!(after["keep"], 1);
        assert!(slot.read().unwrap().is_none());
    }

    #[test]
    fn write_creates_file_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("sub").join("auth.json");
        let slot = JsonKeysSlot::new(p.clone(), &["token"], "auth".into());
        slot.write(br#"{"token":"abc"}"#).unwrap();
        let v: Value = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
        assert_eq!(v["token"], "abc");
    }
}
