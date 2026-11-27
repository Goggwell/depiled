//! Reading and writing collections in Steam's own files, while Steam is closed.
//!
//! Steam keeps collections in `cloudstorage/cloud-storage-namespace-1.json`, a list of
//! `[key, entry]` pairs. When it starts it uploads every key listed in
//! `cloud-storage-namespace-1.modified.json`, so each key changed here is added to that list,
//! otherwise the next cloud sync would quietly bring back the old version.
//!
//! Some Steam builds also keep a copy of the same list in their browser storage (a LevelDB
//! database). When that copy exists it is updated too, so both agree.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde_json::{Map, Value, json};

use crate::model::Collection;
use crate::ops::Op;
use crate::steam::SteamInstall;

const FILE: &str = "cloud-storage-namespace-1.json";
const MODIFIED: &str = "cloud-storage-namespace-1.modified.json";
const NAMESPACES: &str = "cloud-storage-namespaces.json";
const PREFIX: &str = "user-collections.";

pub struct Catalog {
    dir: PathBuf,
    entries: Vec<Value>,
    /// Keys changed since reading, to be listed for upload.
    touched: BTreeSet<String>,
    version: String,
    mirror: Option<mirror::Mirror>,
}

fn now() -> u64 {
    crate::snapshot::now_secs()
}

impl Catalog {
    pub fn read(install: &SteamInstall) -> Result<Self> {
        Self::read_dir(&install.cloudstorage_dir(), mirror::find(install)?)
    }

    fn read_dir(dir: &Path, mirror: Option<mirror::Mirror>) -> Result<Self> {
        let path = dir.join(FILE);
        let entries: Vec<Value> = match std::fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text).with_context(|| format!("{} is not valid", path.display()))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
        };
        // A newer list in the browser storage wins: Steam reads that one first.
        let entries = match &mirror {
            Some(m) if newest(&m.entries) > newest(&entries) => m.entries.clone(),
            _ => entries,
        };
        let version = std::fs::read_to_string(dir.join(NAMESPACES))
            .ok()
            .and_then(|t| serde_json::from_str::<Vec<(u32, String)>>(&t).ok())
            .and_then(|ns| ns.into_iter().find(|(n, _)| *n == 1).map(|(_, v)| v))
            .unwrap_or_else(|| "0".to_owned());
        Ok(Self { dir: dir.to_owned(), entries, touched: BTreeSet::new(), version, mirror })
    }

    pub fn collections(&self) -> Vec<Collection> {
        self.entries
            .iter()
            .filter_map(|item| {
                let entry = item.get(1)?;
                let key = item.get(0)?.as_str()?;
                if !key.starts_with(PREFIX) || entry.get("is_deleted").and_then(Value::as_bool) == Some(true) {
                    return None;
                }
                let value: Value = serde_json::from_str(entry.get("value")?.as_str()?).ok()?;
                let id = value.get("id")?.as_str()?.to_owned();
                if !Collection::is_user_collection_id(&id) {
                    return None;
                }
                Some(Collection {
                    name: value.get("name").and_then(Value::as_str).unwrap_or_default().to_owned(),
                    apps: value.get("added").and_then(Value::as_array).map(|a| a.iter().filter_map(|v| v.as_u64()).map(|v| v as u32).collect()).unwrap_or_default(),
                    dynamic: value.get("filterSpec").is_some(),
                    id,
                })
            })
            .collect()
    }

    fn position(&self, key: &str) -> Option<usize> {
        self.entries.iter().position(|e| e.get(0).and_then(Value::as_str) == Some(key))
    }

    /// The collection's stored value, as an object, if it exists and isn't deleted.
    fn value(&self, key: &str) -> Option<Map<String, Value>> {
        let entry = self.entries.get(self.position(key)?)?.get(1)?;
        serde_json::from_str(entry.get("value")?.as_str()?).ok()
    }

    fn put(&mut self, key: &str, value: Option<Map<String, Value>>) {
        let version = self
            .position(key)
            .and_then(|i| self.entries[i].get(1)?.get("version").cloned())
            .unwrap_or_else(|| Value::String(self.version.clone()));
        let entry = match value {
            Some(v) => json!({ "key": key, "timestamp": now(), "value": Value::Object(v).to_string(), "version": version }),
            None => json!({ "key": key, "timestamp": now(), "is_deleted": true, "version": version }),
        };
        let item = json!([key, entry]);
        match self.position(key) {
            Some(i) => self.entries[i] = item,
            None => self.entries.push(item),
        }
        self.touched.insert(key.to_owned());
    }

    pub fn apply(&mut self, op: &Op) -> Result<()> {
        let key = |id: &str| format!("{PREFIX}{id}");
        match op {
            Op::Create { id, name, apps } => {
                if self.value(&key(id)).is_some() {
                    bail!("collection {id} already exists");
                }
                let mut v = Map::new();
                v.insert("id".into(), json!(id));
                v.insert("name".into(), json!(name));
                v.insert("added".into(), json!(apps));
                v.insert("removed".into(), json!([]));
                self.put(&key(id), Some(v));
            }
            Op::Add { id, apps, .. } | Op::Remove { id, apps, .. } => {
                let mut v = self.value(&key(id)).with_context(|| format!("collection {id} not found"))?;
                let list = |v: &Map<String, Value>, field: &str| -> Vec<u32> {
                    v.get(field).and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.as_u64()).map(|x| x as u32).collect()).unwrap_or_default()
                };
                let (mut added, mut removed) = (list(&v, "added"), list(&v, "removed"));
                // `removed` tells the cloud merge that a game left on purpose, so it isn't
                // brought back from another device's copy.
                if matches!(op, Op::Add { .. }) {
                    added.extend(apps.iter().filter(|a| !added.contains(a)).copied().collect::<Vec<_>>());
                    removed.retain(|a| !apps.contains(a));
                } else {
                    added.retain(|a| !apps.contains(a));
                    removed.extend(apps.iter().filter(|a| !removed.contains(a)).copied().collect::<Vec<_>>());
                }
                v.insert("added".into(), json!(added));
                v.insert("removed".into(), json!(removed));
                self.put(&key(id), Some(v));
            }
            Op::Rename { id, to, .. } => {
                let mut v = self.value(&key(id)).with_context(|| format!("collection {id} not found"))?;
                v.insert("name".into(), json!(to));
                self.put(&key(id), Some(v));
            }
            Op::Delete { id, .. } => {
                if self.value(&key(id)).is_none() {
                    bail!("collection {id} not found");
                }
                self.put(&key(id), None);
            }
        }
        Ok(())
    }

    /// Writes the list, marks the changed keys for upload, and updates the browser-storage copy.
    /// Steam must be closed, or it overwrites the file with what it has in memory.
    pub fn write(&self) -> Result<()> {
        if self.touched.is_empty() {
            return Ok(());
        }
        let text = serde_json::to_string(&self.entries)?;
        write_atomic(&self.dir.join(FILE), text.as_bytes())?;

        let modified_path = self.dir.join(MODIFIED);
        let mut pending: Vec<String> = std::fs::read_to_string(&modified_path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        for key in &self.touched {
            if !pending.contains(key) {
                pending.push(key.clone());
            }
        }
        write_atomic(&modified_path, serde_json::to_string(&pending)?.as_bytes())?;

        if let Some(m) = &self.mirror {
            m.write(&text)?;
        }
        Ok(())
    }
}

fn newest(entries: &[Value]) -> u64 {
    entries.iter().filter_map(|e| e.get(1)?.get("timestamp")?.as_u64()).max().unwrap_or(0)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = path.with_extension("depiled-tmp");
    std::fs::write(&tmp, bytes).with_context(|| format!("writing {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| format!("replacing {}", path.display()))
}

/// A new collection id in Steam's style: `uc-` and twelve letters or digits.
pub fn new_id() -> String {
    use std::hash::{BuildHasher, Hasher};
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos()));
    let mut x = h.finish();
    let mut id = String::from("uc-");
    for _ in 0..12 {
        id.push(ALPHABET[(x % ALPHABET.len() as u64) as usize] as char);
        x = (x / ALPHABET.len() as u64) ^ x.rotate_left(17);
    }
    id
}

/// The copy of the collection list inside Steam's browser storage.
pub mod mirror {
    use super::*;
    use rusty_leveldb::{DB, Options};

    pub struct Mirror {
        pub path: PathBuf,
        key: Vec<u8>,
        utf16: bool,
        pub entries: Vec<Value>,
    }

    fn key(account_id: u32) -> Vec<u8> {
        format!("_https://steamloopback.host\u{0}\u{1}U{account_id}-cloud-storage-namespace-1").into_bytes()
    }

    pub fn candidates(install: &SteamInstall) -> Vec<PathBuf> {
        let mut out = Vec::new();
        if let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
            out.push(local.join("Steam/htmlcache/Default/Local Storage/leveldb"));
            out.push(local.join("Steam/htmlcache/Local Storage/leveldb"));
        }
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            out.push(home.join("Library/Application Support/Steam/config/htmlcache/Default/Local Storage/leveldb"));
            out.push(home.join(".steam/steam/config/htmlcache/Default/Local Storage/leveldb"));
        }
        out.push(install.dir.join("config/htmlcache/Default/Local Storage/leveldb"));
        out.push(install.dir.join("config/htmlcache/Local Storage/leveldb"));
        out
    }

    fn open(path: &Path) -> Result<DB> {
        let options = Options { create_if_missing: false, paranoid_checks: true, ..Options::default() };
        DB::open(path, options).map_err(|e| anyhow::anyhow!("could not open Steam's browser storage at {}: {e}", path.display()))
    }

    /// Opening a LevelDB writes to it, and Steam holds a lock on it while it runs, so lookups
    /// happen on a temporary copy. Only `write` opens the real database.
    fn read_copy(path: &Path, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let tmp = std::env::temp_dir().join(format!("depiled-leveldb-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp)?;
        for entry in std::fs::read_dir(path)?.filter_map(Result::ok) {
            let name = entry.file_name();
            if name != "LOCK" && entry.path().is_file() {
                std::fs::copy(entry.path(), tmp.join(&name))?;
            }
        }
        let value = open(&tmp).map(|mut db| db.get(key).map(|v| v.to_vec()));
        let _ = std::fs::remove_dir_all(&tmp);
        value
    }

    /// The first browser-storage database that holds this account's collection list.
    pub fn find(install: &SteamInstall) -> Result<Option<Mirror>> {
        let key = key(install.account_id);
        for path in candidates(install).into_iter().filter(|p| p.join("CURRENT").is_file()) {
            let Some(raw) = read_copy(&path, &key)? else { continue };
            let (utf16, text) = decode(&raw)?;
            let entries = serde_json::from_str(&text).context("Steam's browser storage holds an unreadable collection list")?;
            return Ok(Some(Mirror { path, key, utf16, entries }));
        }
        Ok(None)
    }

    /// Chromium stores local-storage strings with a one-byte marker: 0 for UTF-16, 1 for Latin-1.
    pub fn decode(raw: &[u8]) -> Result<(bool, String)> {
        match raw.split_first() {
            Some((0, rest)) if rest.len() % 2 == 0 => {
                let units: Vec<u16> = rest.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
                Ok((true, String::from_utf16(&units)?))
            }
            Some((1, rest)) => Ok((false, rest.iter().map(|&b| b as char).collect())),
            _ => bail!("unknown string format in Steam's browser storage"),
        }
    }

    pub fn encode(text: &str, utf16: bool) -> Vec<u8> {
        if utf16 || text.chars().any(|c| c as u32 > 0xFF) {
            let mut out = vec![0];
            out.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
            out
        } else {
            let mut out = vec![1];
            out.extend(text.chars().map(|c| c as u8));
            out
        }
    }

    impl Mirror {
        pub fn write(&self, text: &str) -> Result<()> {
            let mut db = open(&self.path)?;
            db.put(&self.key, &encode(text, self.utf16)).map_err(|e| anyhow::anyhow!("could not update Steam's browser storage: {e}"))?;
            db.flush().map_err(|e| anyhow::anyhow!("could not update Steam's browser storage: {e}"))?;
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(dir: &Path) {
        let value = json!({"id": "uc-AAA", "name": "Planned", "added": [10, 20], "removed": []}).to_string();
        let entries = json!([
            ["user-collections.uc-AAA", {"key": "user-collections.uc-AAA", "timestamp": 1, "value": value, "version": "7"}],
            ["user-collections.hidden", {"key": "user-collections.hidden", "timestamp": 1, "value": "{\"id\":\"hidden\",\"name\":\"\",\"added\":[]}", "version": "7"}],
            ["showcases.0", {"key": "showcases.0", "timestamp": 1, "value": "x", "version": "7"}]
        ]);
        std::fs::write(dir.join(FILE), entries.to_string()).unwrap();
        std::fs::write(dir.join(MODIFIED), "[]").unwrap();
        std::fs::write(dir.join(NAMESPACES), r#"[[1,"9"]]"#).unwrap();
    }

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("depiled-catalog-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn awkward_names_survive_a_save_unchanged() {
        let dir = temp("names");
        sample(&dir);
        let mut cat = Catalog::read_dir(&dir, None).unwrap();
        let names = [r#"Say "hi""#, r"C:\Games\Old", "🏆 Completed", "Café — Ünïcödé 日本語", r"ends with \"];
        for (i, name) in names.iter().enumerate() {
            cat.apply(&Op::Create { id: format!("uc-N{i}"), name: (*name).into(), apps: vec![i as u32 + 1] }).unwrap();
        }
        cat.write().unwrap();
        // Steam's file is still valid JSON, and every name reads back exactly.
        let raw = std::fs::read_to_string(dir.join(FILE)).unwrap();
        assert!(serde_json::from_str::<Value>(&raw).is_ok());
        let again = Catalog::read_dir(&dir, None).unwrap().collections();
        for (i, name) in names.iter().enumerate() {
            assert_eq!(again.iter().find(|c| c.id == format!("uc-N{i}")).unwrap().name, *name);
        }
    }

    #[test]
    fn edits_round_trip_and_are_marked_for_upload() {
        let dir = temp("edit");
        sample(&dir);
        let mut cat = Catalog::read_dir(&dir, None).unwrap();
        assert_eq!(cat.collections().len(), 1, "built-in Hidden is not a user collection");

        cat.apply(&Op::Remove { id: "uc-AAA".into(), name: "Planned".into(), apps: vec![10] }).unwrap();
        cat.apply(&Op::Create { id: "uc-BBB".into(), name: "New".into(), apps: vec![30] }).unwrap();
        cat.apply(&Op::Rename { id: "uc-AAA".into(), from: "Planned".into(), to: "Later".into() }).unwrap();
        cat.write().unwrap();

        let again = Catalog::read_dir(&dir, None).unwrap();
        let cols = again.collections();
        let later = cols.iter().find(|c| c.id == "uc-AAA").unwrap();
        assert_eq!((later.name.as_str(), later.apps.clone()), ("Later", vec![20]));
        assert_eq!(cols.iter().find(|c| c.id == "uc-BBB").unwrap().apps, vec![30]);
        let removed = again.value("user-collections.uc-AAA").unwrap()["removed"].clone();
        assert_eq!(removed, json!([10]));
        // New entries take the namespace version; edited ones keep theirs.
        assert_eq!(again.entries.iter().find(|e| e[0] == "user-collections.uc-BBB").unwrap()[1]["version"], "9");
        let pending: Vec<String> = serde_json::from_str(&std::fs::read_to_string(dir.join(MODIFIED)).unwrap()).unwrap();
        assert_eq!(pending, vec!["user-collections.uc-AAA", "user-collections.uc-BBB"]);
        // Unrelated entries are untouched.
        assert!(again.position("showcases.0").is_some());

        let mut del = Catalog::read_dir(&dir, None).unwrap();
        del.apply(&Op::Delete { id: "uc-BBB".into(), name: "New".into() }).unwrap();
        del.write().unwrap();
        assert!(Catalog::read_dir(&dir, None).unwrap().collections().iter().all(|c| c.id != "uc-BBB"));
    }

    #[test]
    fn mirror_strings_round_trip() {
        for (text, utf16) in [("[1,2]", false), ("[\"Pokémon\"]", false), ("[\"ゲーム\"]", true)] {
            let raw = mirror::encode(text, false);
            assert_eq!(mirror::decode(&raw).unwrap(), (utf16, text.to_owned()));
        }
    }

    #[test]
    fn ids_look_like_steams() {
        let a = new_id();
        assert!(a.starts_with("uc-") && a.len() == 15);
        assert_ne!(a, new_id());
    }
}
