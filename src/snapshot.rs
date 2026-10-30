use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;

use crate::model::{Collection, Snapshot};

pub fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Root folder for snapshots, outside Steam's own directories.
pub fn store_dir() -> Result<PathBuf> {
    Ok(crate::config::dir()?.join("snapshots"))
}

/// Saves a snapshot to disk before any change is made.
///
/// Two layers: a JSON list of collections (drives `undo` against the live client) and a
/// byte-for-byte copy of Steam's cloudstorage folder (manual disaster recovery).
pub fn save(
    dir: &Path,
    label: &str,
    collections: Vec<Collection>,
    cloudstorage: &Path,
) -> Result<PathBuf> {
    let taken_at = now_secs();
    let folder = dir.join(format!("{taken_at}-{}", sanitize(label)));
    let files = folder.join("cloudstorage");
    std::fs::create_dir_all(&files)?;

    for entry in std::fs::read_dir(cloudstorage)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            std::fs::copy(entry.path(), files.join(entry.file_name()))?;
        }
    }

    let snap = Snapshot { taken_at, label: label.to_owned(), collections };
    std::fs::write(folder.join("collections.json"), serde_json::to_string_pretty(&snap)?)?;
    prune(dir, KEEP_SNAPSHOTS)?;
    Ok(folder)
}

const KEEP_SNAPSHOTS: usize = 200;

/// Deletes all but the newest `keep` entries (files or folders) in `dir`, by modified time.
/// Only ever called on folders this app owns.
pub fn prune(dir: &Path, keep: usize) -> Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(Result::ok)
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    entries.sort_by_key(|(t, _)| std::cmp::Reverse(*t));
    for (_, path) in entries.into_iter().skip(keep) {
        if path.is_dir() {
            std::fs::remove_dir_all(path)?;
        } else {
            std::fs::remove_file(path)?;
        }
    }
    Ok(())
}

/// Snapshots newest first.
pub fn list(dir: &Path) -> Result<Vec<(PathBuf, Snapshot)>> {
    let mut out = Vec::new();
    if !dir.exists() {
        return Ok(out);
    }
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        let Ok(text) = std::fs::read_to_string(path.join("collections.json")) else {
            continue;
        };
        out.push((path, serde_json::from_str::<Snapshot>(&text)?));
    }
    out.sort_by_key(|(_, s)| std::cmp::Reverse(s.taken_at));
    Ok(out)
}

fn sanitize(label: &str) -> String {
    label
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_then_list_roundtrips_and_copies_files() {
        let tmp = std::env::temp_dir().join(format!("sc-test-{}", now_secs()));
        let cloud = tmp.join("cloud");
        std::fs::create_dir_all(&cloud).unwrap();
        std::fs::write(cloud.join("cloud-storage-namespace-1.json"), "[]").unwrap();

        let cols = vec![Collection { id: "uc-1".into(), name: "A".into(), apps: vec![10, 20], dynamic: false }];
        let folder = save(&tmp.join("snaps"), "before: rules", cols.clone(), &cloud).unwrap();

        assert!(folder.join("cloudstorage/cloud-storage-namespace-1.json").exists());
        let listed = list(&tmp.join("snaps")).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].1.collections, cols);
        std::fs::remove_dir_all(tmp).unwrap();
    }
}
