use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::rules::Rule;
use crate::snapshot;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Extra place to mirror backups, e.g. a OneDrive or Dropbox folder, for other devices.
    pub backup_dir: Option<PathBuf>,
    #[serde(default, rename = "rule")]
    pub rules: Vec<Rule>,
}

impl Config {
    pub fn parse(text: &str) -> Result<Self> {
        let cfg: Config = toml::from_str(text).context("config.toml is not valid")?;
        cfg.validate()?;
        Ok(cfg)
    }

    pub fn validate(&self) -> Result<()> {
        if self.rules.is_empty() {
            bail!("config has no [[rule]] entries");
        }
        self.rules.iter().try_for_each(Rule::validate)
    }
}

pub const DEFAULT_CONFIG: &str = r#"# Depiled configuration.
# Rules run top to bottom. Every condition inside a rule must hold for a game to match.
# Rules that share a `group` are exclusive: the first match wins and the game is moved
# out of the group's other collections. Collection names are yours to choose; existing
# collections are matched ignoring case, missing ones are created.

# backup_dir = "C:/Users/you/OneDrive/depiled"

[[rule]]
collection = "Completed"
group = "status"
sticky = true                    # games already here are never moved out
all_achievements = true          # every achievement unlocked, read from Steam itself

[[rule]]
collection = "Unfinished"
group = "status"
played = true
not_played_for_days = 7          # change this to taste

[[rule]]
collection = "Started"
group = "status"
played = true

# Games you've never launched. Unfinished is only for games you started and left.
[[rule]]
collection = "Planned"
group = "status"
played = false
# not_in_collections = ["Won't play"]   # keep games you've filed elsewhere out of Planned

# Extra rules add games to collections too. Examples:
# [[rule]]
# collection = "Cozy solo games"
# has_flags = ["Single-player", "Steam Cloud"]
# min_review_percent = 90
# sync = true                    # also remove games that stop matching
#
# [[rule]]
# collection = "Scary"
# has_any_tags = ["Horror", "Psychological Horror"]   # genres and tags, by name
"#;

const APP_DIR: &str = "depiled";

/// Where the app keeps its own files (settings, rules, snapshots), outside Steam's folders.
pub fn dir() -> Result<PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from).context("APPDATA is not set")?
    } else {
        let home = std::env::var_os("HOME").map(PathBuf::from).context("HOME is not set")?;
        if cfg!(target_os = "macos") {
            home.join("Library/Application Support")
        } else {
            std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".config"))
        }
    };
    let dir = base.join(APP_DIR);
    // The app was called steam-collections before; carry its files over once.
    let old = base.join("steam-collections");
    if !dir.exists() && old.is_dir() {
        let _ = std::fs::rename(&old, &dir);
    }
    Ok(dir)
}

pub fn path() -> Result<PathBuf> {
    Ok(dir()?.join("config.toml"))
}

/// Loads the config, writing the default one first if this is a fresh install.
pub fn load() -> Result<(Config, String)> {
    let path = path()?;
    if !path.exists() {
        init()?;
    }
    let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    Ok((Config::parse(&text)?, text))
}

/// Writes the default config if none exists. Never overwrites.
pub fn init() -> Result<PathBuf> {
    let path = path()?;
    if path.exists() {
        bail!("{} already exists", path.display());
    }
    std::fs::create_dir_all(path.parent().context("no parent")?)?;
    std::fs::write(&path, DEFAULT_CONFIG)?;
    Ok(path)
}

/// Replaces the config after saving the previous one into `config-backups`.
pub fn replace_with_backup(new_text: &str) -> Result<()> {
    Config::parse(new_text)?;
    let path = path()?;
    if let Ok(old) = std::fs::read_to_string(&path) {
        let backups = dir()?.join("config-backups");
        std::fs::create_dir_all(&backups)?;
        std::fs::write(backups.join(format!("config-{}.toml", snapshot::now_secs())), old)?;
        snapshot::prune(&backups, 20)?;
    }
    std::fs::write(path, new_text)?;
    Ok(())
}

pub fn ensure_dir(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path).with_context(|| format!("creating {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_parses_and_has_the_status_group_in_priority_order() {
        let cfg = Config::parse(DEFAULT_CONFIG).unwrap();
        let names: Vec<_> = cfg.rules.iter().map(|r| r.collection.as_str()).collect();
        assert_eq!(names, ["Completed", "Unfinished", "Started", "Planned"]);
        assert_eq!(cfg.rules[1].not_played_for_days, Some(7));
    }

    #[test]
    fn typos_in_rule_fields_are_rejected_not_ignored() {
        let bad = "[[rule]]\ncollection = \"A\"\nnot_played_for_dayz = 3\n";
        assert!(Config::parse(bad).is_err());
    }

    #[test]
    fn unknown_flags_are_rejected() {
        let bad = "[[rule]]\ncollection = \"A\"\nhas_flags = [\"Nope\"]\n";
        assert!(Config::parse(bad).is_err());
    }
}
