//! Plaintext (0600) JSON vault. Opt-in fallback for machines without a credential store.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

use anyhow::{Context, Result};
use base64::Engine;
use serde::{Deserialize, Serialize};

use super::Vault;
use crate::core::fsutil::{read_opt, write_atomic};
use crate::core::model::SecretBlob;

#[derive(Default, Serialize, Deserialize)]
struct FileVaultDoc {
    #[serde(default)]
    entries: BTreeMap<String, FileEntry>,
}

#[derive(Serialize, Deserialize)]
struct FileEntry {
    label: String,
    data: String,
}

pub struct FileVault {
    path: PathBuf,
    io: Mutex<()>,
}

impl FileVault {
    pub fn new(path: PathBuf) -> FileVault {
        FileVault { path, io: Mutex::new(()) }
    }

    fn load(&self) -> Result<FileVaultDoc> {
        match read_opt(&self.path)? {
            Some(b) => serde_json::from_slice(&b).with_context(|| format!("parsing {}", self.path.display())),
            None => Ok(FileVaultDoc::default()),
        }
    }

    fn store(&self, doc: &FileVaultDoc) -> Result<()> {
        write_atomic(&self.path, &serde_json::to_vec_pretty(doc)?, 0o600)
    }
}

impl Vault for FileVault {
    fn name(&self) -> &'static str {
        "file"
    }

    fn get(&self, key: &str) -> Result<Option<SecretBlob>> {
        let _g = self.io.lock().unwrap();
        let doc = self.load()?;
        match doc.entries.get(key) {
            Some(e) => {
                let bytes = base64::engine::general_purpose::STANDARD.decode(&e.data)?;
                Ok(Some(SecretBlob::new(bytes)))
            }
            None => Ok(None),
        }
    }

    fn put(&self, key: &str, label: &str, blob: &SecretBlob) -> Result<()> {
        let _g = self.io.lock().unwrap();
        let mut doc = self.load()?;
        doc.entries.insert(
            key.to_string(),
            FileEntry {
                label: label.to_string(),
                data: base64::engine::general_purpose::STANDARD.encode(blob.as_bytes()),
            },
        );
        self.store(&doc)
    }

    fn delete(&self, key: &str) -> Result<()> {
        let _g = self.io.lock().unwrap();
        let mut doc = self.load()?;
        doc.entries.remove(key);
        self.store(&doc)
    }

    fn health(&self) -> Result<String> {
        Ok(format!("plaintext file {}", self.path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let v = FileVault::new(dir.path().join("vault.json"));
        assert!(v.get("p:a").unwrap().is_none());
        v.put("p:a", "A", &SecretBlob::new(b"{\"t\":1}".to_vec())).unwrap();
        assert_eq!(v.get("p:a").unwrap().unwrap().as_bytes(), b"{\"t\":1}");
        v.delete("p:a").unwrap();
        assert!(v.get("p:a").unwrap().is_none());
    }
}
