//! Generic `Provider` for CLIs whose login is "these slots plus a way to learn the identity".
//! Most providers are just a `SlotProvider` value.

use anyhow::{Context, Result, bail};
use base64::Engine;
use serde::{Deserialize, Serialize};

use super::identity::{Cmd, IdentityResolver, run_cmd};
use super::slots::Slot;
use super::util::fp::fnv1a;
use super::{Provider, ProviderMeta, default_preflight};
use crate::core::cx::Cx;
use crate::core::model::{Captured, Identity, SecretBlob, Warning};

pub struct SlotProvider {
    pub meta: ProviderMeta,
    /// Build the slots for this environment (paths depend on OS/home/env).
    pub slots: fn(&Cx) -> Vec<Box<dyn Slot>>,
    pub identity: IdentityResolver,
    /// Optional post-switch check via the CLI itself.
    pub verify: Option<Cmd>,
    pub extra_preflight: Option<fn(&Cx) -> Vec<Warning>>,
}

const BLOB_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct Blob {
    v: u32,
    slots: Vec<Option<String>>,
}

impl SlotProvider {
    fn read_all(&self, slots: &[Box<dyn Slot>]) -> Result<Vec<Option<Vec<u8>>>> {
        slots.iter().map(|s| s.read().with_context(|| format!("reading {}", s.describe()))).collect()
    }

    fn encode(data: &[Option<Vec<u8>>]) -> SecretBlob {
        let b64 = base64::engine::general_purpose::STANDARD;
        let blob = Blob { v: BLOB_VERSION, slots: data.iter().map(|d| d.as_ref().map(|b| b64.encode(b))).collect() };
        SecretBlob::new(serde_json::to_vec(&blob).expect("blob json"))
    }

    fn decode(secret: &SecretBlob, expected_slots: usize) -> Result<Vec<Option<Vec<u8>>>> {
        let blob: Blob = serde_json::from_slice(secret.as_bytes()).context("saved credential is not readable")?;
        if blob.v != BLOB_VERSION {
            bail!("saved credential has unsupported format version {}", blob.v);
        }
        if blob.slots.len() != expected_slots {
            bail!("saved credential has {} parts, expected {}", blob.slots.len(), expected_slots);
        }
        let b64 = base64::engine::general_purpose::STANDARD;
        blob.slots
            .iter()
            .map(|s| s.as_ref().map(|v| b64.decode(v).context("saved credential is corrupt")).transpose())
            .collect()
    }

    fn write_all(slots: &[Box<dyn Slot>], data: &[Option<Vec<u8>>]) -> Result<()> {
        let backup: Vec<Option<Vec<u8>>> = slots.iter().map(|s| s.read().unwrap_or(None)).collect();
        for (i, slot) in slots.iter().enumerate() {
            let r = match &data[i] {
                Some(bytes) => slot.write(bytes),
                None => slot.clear(),
            };
            if let Err(e) = r {
                // roll back the slots already written (indices before i)
                for (j, prev) in backup.iter().enumerate().take(i) {
                    let _ = match prev {
                        Some(b) => slots[j].write(b),
                        None => slots[j].clear(),
                    };
                }
                return Err(e).with_context(|| format!("writing {}", slot.describe()));
            }
        }
        Ok(())
    }
}

impl Provider for SlotProvider {
    fn meta(&self) -> &ProviderMeta {
        &self.meta
    }

    fn slot_descriptions(&self, cx: &Cx) -> Vec<String> {
        (self.slots)(cx).iter().map(|s| s.describe()).collect()
    }

    fn fingerprint(&self, cx: &Cx) -> Result<Option<u64>> {
        let slots = (self.slots)(cx);
        Ok(Some(fnv1a(&self.read_all(&slots)?)))
    }

    fn preflight(&self, cx: &Cx) -> Vec<Warning> {
        let mut w = default_preflight(&self.meta, cx);
        if let Some(f) = self.extra_preflight {
            w.extend(f(cx));
        }
        w
    }

    fn live_identity(&self, cx: &Cx) -> Result<Option<Identity>> {
        let slots = (self.slots)(cx);
        let data = self.read_all(&slots)?;
        if data.iter().all(Option::is_none) {
            return Ok(None);
        }
        self.identity.resolve(cx, &data)
    }

    fn capture(&self, cx: &Cx) -> Result<Option<Captured>> {
        let slots = (self.slots)(cx);
        let data = self.read_all(&slots)?;
        if data.iter().all(Option::is_none) {
            return Ok(None);
        }
        let (identity, identity_error) = match self.identity.resolve(cx, &data) {
            Ok(i) => (i, None),
            Err(e) => (None, Some(format!("{e:#}"))),
        };
        Ok(Some(Captured { identity, secret: Self::encode(&data), identity_error }))
    }

    fn activate(&self, cx: &Cx, secret: &SecretBlob, _identity: &Identity) -> Result<()> {
        let slots = (self.slots)(cx);
        let data = Self::decode(secret, slots.len())?;
        Self::write_all(&slots, &data)
    }

    fn clear(&self, cx: &Cx) -> Result<()> {
        for slot in (self.slots)(cx) {
            slot.clear().with_context(|| format!("clearing {}", slot.describe()))?;
        }
        Ok(())
    }

    fn verify(&self, cx: &Cx, expected: &Identity) -> Result<bool> {
        match &self.verify {
            Some(cmd) => Ok(run_cmd(cx, cmd)?.map(|l| l.same_as(expected)).unwrap_or(false)),
            None => {
                if self.identity.is_offline() {
                    Ok(self.live_identity(cx)?.map(|l| l.same_as(expected)).unwrap_or(false))
                } else {
                    // Network identity, no dedicated verify: trust the write (the slot re-reads as written).
                    Ok(true)
                }
            }
        }
    }
}

#[cfg(test)]
pub mod testing {
    //! A slot-backed provider over a temp home, for orchestration tests.
    use super::*;
    use crate::core::model::Strategy;
    use crate::providers::Tier;
    use crate::providers::slots::JsonKeysSlot;

    pub fn json_file_provider() -> SlotProvider {
        SlotProvider {
            meta: ProviderMeta {
                id: "mock",
                name: "Mock CLI",
                strategy: Strategy::SlotSwap,
                tier: Tier::Supported,
                binaries: &[],
                process_names: &[],
                env_shadow: &["MOCK_TOKEN"],
                restart_hint: None,
                notes: "test",
                login: &["mock", "login"],
            },
            slots: |cx| {
                vec![Box::new(JsonKeysSlot::new(cx.home().join("mock.json"), &["token", "user"], "mock.json".into()))]
            },
            identity: IdentityResolver::JsonPointer { slot: 0, pointer: "/user", is_email: true, extra: &[] },
            verify: None,
            extra_preflight: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::json_file_provider;
    use super::*;
    use crate::core::cx::Os;

    #[test]
    fn capture_activate_round_trip_preserves_other_keys() {
        let dir = tempfile::tempdir().unwrap();
        let cx = Cx::test(dir.path(), Os::Linux);
        let p = json_file_provider();
        let file = dir.path().join("mock.json");
        std::fs::write(&file, r#"{"token":"t-a","user":"a@x.io","theme":"dark"}"#).unwrap();

        let cap_a = p.capture(&cx).unwrap().unwrap();
        assert_eq!(cap_a.identity.as_ref().unwrap().id, "a@x.io");
        let fp_a = p.fingerprint(&cx).unwrap();

        std::fs::write(&file, r#"{"token":"t-b","user":"b@x.io","theme":"dark"}"#).unwrap();
        assert_ne!(p.fingerprint(&cx).unwrap(), fp_a);
        assert_eq!(p.live_identity(&cx).unwrap().unwrap().id, "b@x.io");

        p.activate(&cx, &cap_a.secret, cap_a.identity.as_ref().unwrap()).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
        assert_eq!(v["token"], "t-a");
        assert_eq!(v["user"], "a@x.io");
        assert_eq!(v["theme"], "dark");
        assert!(p.verify(&cx, cap_a.identity.as_ref().unwrap()).unwrap());

        p.clear(&cx).unwrap();
        assert!(p.live_identity(&cx).unwrap().is_none());
        assert!(p.capture(&cx).unwrap().is_none());
    }

    #[test]
    fn preflight_reports_env_shadow() {
        let dir = tempfile::tempdir().unwrap();
        let mut cx = Cx::test(dir.path(), Os::Linux);
        let p = json_file_provider();
        assert!(p.preflight(&cx).is_empty());
        cx.set_env("MOCK_TOKEN", "x");
        assert_eq!(p.preflight(&cx)[0].code, "env-shadow");
    }

    #[test]
    fn rejects_foreign_blob() {
        let dir = tempfile::tempdir().unwrap();
        let cx = Cx::test(dir.path(), Os::Linux);
        let p = json_file_provider();
        let bad = SecretBlob::new(br#"{"v":1,"slots":[null,null]}"#.to_vec());
        assert!(p.activate(&cx, &bad, &Identity::new("x")).is_err());
    }
}
