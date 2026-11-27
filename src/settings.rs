//! App preferences that aren't rules: how automatic organizing should be, which online extras
//! to fetch, and where updates come from.

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::config;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Automation {
    /// Never organize on its own.
    Off,
    /// Notice when something should move and offer to apply it.
    Notify,
    /// Apply the rules without asking. Every change is still snapshotted, so it can be undone.
    Auto,
}

/// Which builds the app offers to update to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    /// Numbered releases.
    #[default]
    Stable,
    /// The newest build of the main branch, replaced on every change.
    Nightly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    /// `None` until the user has chosen, so the app can ask once.
    #[serde(default)]
    pub automation: Option<Automation>,
    /// Online extras on or off (completion times, achievement progress, prices and
    /// ProtonDB). `None` until the user has chosen.
    #[serde(default)]
    pub online_stats: Option<bool>,
    /// Two-letter country for store prices; `None` follows this computer's region.
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default = "yes")]
    pub online_achievements: bool,
    #[serde(default = "yes")]
    pub online_prices: bool,
    #[serde(default = "yes")]
    pub online_protondb: bool,
    #[serde(default = "yes")]
    pub online_hunters: bool,
    /// Start Steam again after closing it to save.
    #[serde(default = "yes")]
    pub reopen_steam: bool,
    /// Look for a new version when the app starts.
    #[serde(default = "yes")]
    pub check_updates: bool,
    #[serde(default)]
    pub update_channel: Channel,
}

fn yes() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            automation: None,
            online_stats: None,
            region: None,
            online_achievements: true,
            online_prices: true,
            online_protondb: true,
            online_hunters: true,
            reopen_steam: true,
            check_updates: true,
            update_channel: Channel::Stable,
        }
    }
}

fn path() -> Result<PathBuf> {
    Ok(config::dir()?.join("settings.json"))
}

pub fn load_from(path: &Path) -> Result<Settings> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(serde_json::from_str(&text).unwrap_or_default()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Settings::default()),
        Err(e) => Err(e.into()),
    }
}

pub fn save_to(path: &Path, settings: &Settings) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(settings)?)?;
    std::fs::rename(tmp, path)?;
    Ok(())
}

pub fn load() -> Result<Settings> {
    load_from(&path()?)
}

pub fn save(settings: &Settings) -> Result<()> {
    save_to(&path()?, settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_means_the_user_has_not_chosen_yet() {
        let file = std::env::temp_dir().join("sc-settings-missing-xyz").join("settings.json");
        assert_eq!(load_from(&file).unwrap().automation, None);
    }

    #[test]
    fn choice_roundtrips_and_serializes_in_plain_words() {
        let dir = std::env::temp_dir().join(format!("sc-settings-{}", crate::snapshot::now_secs()));
        let file = dir.join("settings.json");
        let s = Settings { automation: Some(Automation::Notify), ..Settings::default() };
        save_to(&file, &s).unwrap();
        assert!(std::fs::read_to_string(&file).unwrap().contains("\"notify\""));
        assert_eq!(load_from(&file).unwrap(), s);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_corrupt_file_falls_back_to_defaults_instead_of_failing() {
        let dir = std::env::temp_dir().join(format!("sc-settings-bad-{}", crate::snapshot::now_secs()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("settings.json");
        std::fs::write(&file, "{ not json").unwrap();
        assert_eq!(load_from(&file).unwrap(), Settings::default());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
