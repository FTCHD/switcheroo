//! Lines matching a predicate inside a line-oriented config (`.npmrc`, `.netrc`) are the slot;
//! all other lines stay exactly where they were.

use std::path::PathBuf;

use anyhow::Result;

use super::Slot;
use crate::core::fsutil::{read_opt, write_atomic};

pub struct LinesSlot {
    pub path: PathBuf,
    pub matcher: fn(&str) -> bool,
    pub display: String,
}

impl LinesSlot {
    pub fn new(path: PathBuf, matcher: fn(&str) -> bool, display: String) -> LinesSlot {
        LinesSlot { path, matcher, display }
    }

    fn lines(&self) -> Result<Option<Vec<String>>> {
        Ok(read_opt(&self.path)?.map(|b| String::from_utf8_lossy(&b).lines().map(str::to_string).collect()))
    }

    fn replace(&self, new_lines: &[String]) -> Result<()> {
        let existing = self.lines()?.unwrap_or_default();
        let mut out: Vec<String> = Vec::with_capacity(existing.len() + new_lines.len());
        let mut inserted = false;
        for line in existing {
            if (self.matcher)(&line) {
                if !inserted {
                    out.extend(new_lines.iter().cloned());
                    inserted = true;
                }
            } else {
                out.push(line);
            }
        }
        if !inserted {
            out.extend(new_lines.iter().cloned());
        }
        let mut text = out.join("\n");
        if !text.is_empty() {
            text.push('\n');
        }
        write_atomic(&self.path, text.as_bytes(), 0o600)
    }
}

impl Slot for LinesSlot {
    fn read(&self) -> Result<Option<Vec<u8>>> {
        let Some(lines) = self.lines()? else { return Ok(None) };
        let matched: Vec<String> = lines.into_iter().filter(|l| (self.matcher)(l)).collect();
        if matched.is_empty() {
            return Ok(None);
        }
        Ok(Some(matched.join("\n").into_bytes()))
    }

    fn write(&self, data: &[u8]) -> Result<()> {
        let new_lines: Vec<String> =
            String::from_utf8_lossy(data).lines().filter(|l| (self.matcher)(l)).map(str::to_string).collect();
        self.replace(&new_lines)
    }

    fn clear(&self) -> Result<()> {
        if self.lines()?.is_none() {
            return Ok(());
        }
        self.replace(&[])
    }

    fn describe(&self) -> String {
        self.display.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_token(l: &str) -> bool {
        l.starts_with("//registry.npmjs.org/:_authToken=")
    }

    #[test]
    fn swaps_only_matching_lines_in_place() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join(".npmrc");
        std::fs::write(
            &p,
            "registry=https://registry.npmjs.org/\n//registry.npmjs.org/:_authToken=OLD\nsave-exact=true\n",
        )
        .unwrap();
        let slot = LinesSlot::new(p.clone(), is_token, "~/.npmrc".into());
        assert_eq!(slot.read().unwrap().unwrap(), b"//registry.npmjs.org/:_authToken=OLD");
        slot.write(b"//registry.npmjs.org/:_authToken=NEW").unwrap();
        assert_eq!(
            std::fs::read_to_string(&p).unwrap(),
            "registry=https://registry.npmjs.org/\n//registry.npmjs.org/:_authToken=NEW\nsave-exact=true\n"
        );
        slot.clear().unwrap();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "registry=https://registry.npmjs.org/\nsave-exact=true\n");
        assert!(slot.read().unwrap().is_none());
        slot.write(b"//registry.npmjs.org/:_authToken=AGAIN\nignored=1").unwrap();
        assert!(std::fs::read_to_string(&p).unwrap().ends_with("//registry.npmjs.org/:_authToken=AGAIN\n"));
        assert!(!std::fs::read_to_string(&p).unwrap().contains("ignored"));
    }
}
