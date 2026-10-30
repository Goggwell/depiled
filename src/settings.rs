//! App preferences that aren't rules: currently just how automatic organizing should be.

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

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    /// `None` until the user has chosen, so the app can ask once.
    #[serde(default)]
    pub automation: Option<Automation>,
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
        let s = Settings { automation: Some(Automation::Notify) };
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
