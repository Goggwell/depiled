use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::model::Collection;
use crate::snapshot;

const VERSION: u32 = 1;

/// A portable backup: collections plus the rules that manage them. Moves between devices.
#[derive(Debug, Serialize, Deserialize)]
pub struct Bundle {
    pub version: u32,
    pub exported_at: u64,
    pub collections: Vec<Collection>,
    pub config: Option<String>,
}

pub fn write(path: &Path, collections: Vec<Collection>, config: Option<String>) -> Result<()> {
    let bundle = Bundle { version: VERSION, exported_at: snapshot::now_secs(), collections, config };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(&bundle)?)
        .with_context(|| format!("writing {}", path.display()))
}

pub fn read(path: &Path) -> Result<Bundle> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let bundle: Bundle = serde_json::from_str(&text).context("not a Depiled backup file")?;
    if bundle.version > VERSION {
        bail!("backup was made by a newer version (format {})", bundle.version);
    }
    Ok(bundle)
}

/// Timestamped bundle in `dir`, keeping the newest `keep`.
pub fn auto_backup(dir: &Path, collections: Vec<Collection>, config: Option<String>, keep: usize) -> Result<PathBuf> {
    let path = dir.join(format!("depiled-{}.json", snapshot::now_secs()));
    write(&path, collections, config)?;
    snapshot::prune(dir, keep)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_roundtrips_and_old_backups_are_pruned() {
        let tmp = std::env::temp_dir().join(format!("sc-bundle-{}", snapshot::now_secs()));
        let cols = vec![Collection { id: "uc-1".into(), name: "A".into(), apps: vec![1], dynamic: false }];
        let path = auto_backup(&tmp, cols.clone(), Some("x".into()), 3).unwrap();
        let back = read(&path).unwrap();
        assert_eq!(back.collections, cols);
        assert_eq!(back.config.as_deref(), Some("x"));
        std::fs::remove_dir_all(tmp).unwrap();
    }
}
