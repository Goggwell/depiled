use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::model::Collection;

const STEAM64_BASE: u64 = 76_561_197_960_265_728;

pub struct SteamInstall {
    pub dir: PathBuf,
    pub account_id: u32,
}

#[cfg(windows)]
fn steam_dir() -> Result<PathBuf> {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;
    let key = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(r"Software\Valve\Steam")
        .context("Steam registry key not found")?;
    let dir: String = key.get_value("SteamPath")?;
    Ok(PathBuf::from(dir))
}

#[cfg(not(windows))]
fn steam_dir() -> Result<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from).context("HOME is not set")?;
    let candidates = if cfg!(target_os = "macos") {
        vec![home.join("Library/Application Support/Steam")]
    } else {
        vec![home.join(".steam/steam"), home.join(".local/share/Steam")]
    };
    candidates.into_iter().find(|d| d.join("userdata").is_dir()).context("Steam folder not found")
}

impl SteamInstall {
    /// Locates Steam and picks the most recently active account folder.
    pub fn detect() -> Result<Self> {
        let dir = steam_dir()?;

        let userdata = dir.join("userdata");
        let mut best: Option<(std::time::SystemTime, u32)> = None;
        for entry in std::fs::read_dir(&userdata).context("no userdata folder")? {
            let entry = entry?;
            let Ok(id) = entry.file_name().to_string_lossy().parse::<u32>() else {
                continue;
            };
            let modified = entry.metadata()?.modified()?;
            if best.is_none_or(|(t, _)| modified > t) {
                best = Some((modified, id));
            }
        }
        let (_, account_id) = best.context("no Steam account folders found")?;
        Ok(Self { dir, account_id })
    }

    pub fn steam64(&self) -> u64 {
        STEAM64_BASE + u64::from(self.account_id)
    }

    pub fn config_dir(&self) -> PathBuf {
        self.dir
            .join("userdata")
            .join(self.account_id.to_string())
            .join("config")
    }

    pub fn cloudstorage_dir(&self) -> PathBuf {
        self.config_dir().join("cloudstorage")
    }

    /// The game currently running, as reported by the Steam client (0 means none).
    #[cfg(windows)]
    pub fn running_app_id(&self) -> Result<u32> {
        use winreg::RegKey;
        use winreg::enums::HKEY_CURRENT_USER;
        let key = RegKey::predef(HKEY_CURRENT_USER).open_subkey(r"Software\Valve\Steam")?;
        Ok(key.get_value("RunningAppID")?)
    }

    /// The game currently running, as reported by the Steam client (0 means none).
    #[cfg(not(windows))]
    pub fn running_app_id(&self) -> Result<u32> {
        let text = std::fs::read_to_string(self.dir.join("registry.vdf")).context("reading registry.vdf")?;
        Ok(parse_running_app_id(&text))
    }
}

/// Pulls `"RunningAppID"  "123"` out of registry.vdf.
#[cfg_attr(windows, allow(dead_code))]
fn parse_running_app_id(text: &str) -> u32 {
    text.lines()
        .find(|l| l.contains("\"RunningAppID\""))
        .and_then(|l| l.split('"').nth(3))
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

pub fn steam_is_running() -> Result<bool> {
    use sysinfo::{ProcessesToUpdate, System};
    let name = if cfg!(windows) {
        "steam.exe"
    } else if cfg!(target_os = "macos") {
        "steam_osx"
    } else {
        "steam"
    };
    let mut sys = System::new()?;
    sys.refresh_processes(ProcessesToUpdate::All, true);
    Ok(sys.processes_by_exact_name(name.as_ref()).next().is_some())
}
#[derive(Deserialize)]
struct CloudEntry {
    key: String,
    value: Option<String>,
    #[serde(default)]
    is_deleted: bool,
}

#[derive(Deserialize)]
struct RawCollection {
    id: String,
    name: String,
    #[serde(default)]
    added: Vec<u32>,
    #[serde(default, rename = "filterSpec")]
    filter_spec: Option<serde_json::Value>,
}

/// Read-only parse of the on-disk collections. Never used for writing: Steam owns the merge
/// and version bookkeeping, so edits go through the running client instead.
pub fn read_collections_from_disk(cloudstorage: &Path) -> Result<Vec<Collection>> {
    let path = cloudstorage.join("cloud-storage-namespace-1.json");
    let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let entries: Vec<(String, CloudEntry)> = serde_json::from_str(&text)?;

    let mut out = Vec::new();
    for (key, entry) in entries {
        if !key.starts_with("user-collections.") || entry.is_deleted {
            continue;
        }
        let Some(value) = entry.value else { continue };
        let raw: RawCollection = serde_json::from_str(&value)
            .with_context(|| format!("collection entry {}", entry.key))?;
        if !Collection::is_user_collection_id(&raw.id) {
            continue;
        }
        out.push(Collection {
            id: raw.id,
            name: raw.name,
            apps: raw.added,
            dynamic: raw.filter_spec.is_some(),
        });
    }
    Ok(out)
}

pub fn require_closed() -> Result<()> {
    if steam_is_running()? {
        bail!("Steam is running; close it first");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse_running_app_id;

    #[test]
    fn reads_running_app_id() {
        let vdf = "\"Registry\"\n{\n\t\"Steam\"\n\t{\n\t\t\"RunningAppID\"\t\t\"620\"\n\t}\n}\n";
        assert_eq!(parse_running_app_id(vdf), 620);
        assert_eq!(parse_running_app_id("nothing here"), 0);
    }
}