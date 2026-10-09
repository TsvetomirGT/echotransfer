//! `/.echotransfer/manifest.json` on the card: records every file this app wrote.
//! Only files listed here are ever deleted or overwritten.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Result;

const DIR: &str = ".echotransfer";
const FILE: &str = "manifest.json";
pub const VERSION: u32 = 1;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Manifest {
    pub version: u32,
    pub server: String,
    pub user: String,
    /// Album ids selected for this card.
    pub selection: BTreeSet<String>,
    /// Subsonic song id -> entry.
    pub entries: BTreeMap<String, Entry>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Entry {
    /// Relative to card root, forward slashes.
    pub rel_path: String,
    pub album_id: String,
    /// Hash of server metadata (size, suffix, tags) at download time.
    pub fingerprint: String,
    pub profile: String,
    pub cover_art: Option<String>,
    pub cover_size: u32,
    /// Size of the file as written to the card.
    pub local_size: u64,
    pub synced_at: u64,
}

pub fn manifest_path(root: &Path) -> PathBuf {
    root.join(DIR).join(FILE)
}

/// Resolve a manifest-relative path against the card root.
pub fn abs_path(root: &Path, rel: &str) -> PathBuf {
    rel.split('/').fold(root.to_path_buf(), |p, c| p.join(c))
}

impl Manifest {
    pub fn load(root: &Path) -> Result<Self> {
        match std::fs::read(manifest_path(root)) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self {
                version: VERSION,
                ..Default::default()
            }),
            Err(e) => Err(e.into()),
        }
    }

    /// Atomic write: tmp file + fsync + rename.
    pub fn save(&self, root: &Path) -> Result<()> {
        use std::io::Write;
        let path = manifest_path(root);
        let dir = path.parent().expect("manifest has parent");
        std::fs::create_dir_all(dir)?;
        let tmp = dir.join(format!("{FILE}.tmp"));
        {
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(&serde_json::to_vec(self)?)?;
            f.sync_all()?;
        }
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }

    pub fn exists(root: &Path) -> bool {
        manifest_path(root).exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_manifest_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let m = Manifest::load(dir.path()).unwrap();
        assert!(m.entries.is_empty());
        assert_eq!(m.version, VERSION);
        assert!(!Manifest::exists(dir.path()));
    }

    #[test]
    fn roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let mut m = Manifest {
            version: VERSION,
            server: "https://x/".into(),
            user: "u".into(),
            ..Default::default()
        };
        m.selection.insert("al1".into());
        m.entries.insert(
            "s1".into(),
            Entry {
                rel_path: "Music/A/B/01 C.mp3".into(),
                album_id: "al1".into(),
                local_size: 42,
                ..Default::default()
            },
        );
        m.save(dir.path()).unwrap();
        let back = Manifest::load(dir.path()).unwrap();
        assert_eq!(back.entries["s1"], m.entries["s1"]);
        assert!(back.selection.contains("al1"));
        assert!(!dir.path().join(".echotransfer/manifest.json.tmp").exists());
    }

    #[test]
    fn abs_path_joins_components() {
        let p = abs_path(Path::new("/Volumes/CARD"), "Music/A/x.mp3");
        assert_eq!(p, PathBuf::from("/Volumes/CARD/Music/A/x.mp3"));
    }
}
