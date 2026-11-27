use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

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

// ---- one folder per Steam account ------------------------------------------------------

/// Files that belong to one Steam account: unsaved changes, history, rules, collection colours
/// and achievement progress. Settings and the shared online caches (prices, ratings, completion
/// times) stay at the top, since they're the same whoever is signed in.
const ACCOUNT_FILES: [&str; 7] = ["draft.json", "colors.json", "smart-collections.json", "config.toml", "config-backups", "snapshots", "profile-achievements.json"];

/// The account the app is working on, 0 until a session or command picks one.
static ACCOUNT: AtomicU32 = AtomicU32::new(0);

/// Points the account's files at `account_id`. The session does this when it opens.
pub fn use_account(account_id: u32) {
    ACCOUNT.store(account_id, Ordering::Relaxed);
}

/// Where the current account's own files are.
pub fn account_dir() -> Result<PathBuf> {
    let mut id = ACCOUNT.load(Ordering::Relaxed);
    if id == 0 {
        id = crate::steam::SteamInstall::detect()?.account_id;
        use_account(id);
    }
    dir_for(id)
}

/// Where one account's own files are. Files left at the top by older versions, which only
/// knew one account, are first moved to the account they were made for.
pub fn dir_for(account_id: u32) -> Result<PathBuf> {
    static MOVED: std::sync::Once = std::sync::Once::new();
    let dir = dir()?;
    MOVED.call_once(|| {
        if let Err(e) = move_legacy(&dir) {
            eprintln!("couldn't move older files into an account folder: {e:#}");
        }
    });
    Ok(dir.join("accounts").join(account_id.to_string()))
}

fn move_legacy(dir: &Path) -> Result<()> {
    let legacy: Vec<PathBuf> = ACCOUNT_FILES.iter().map(|n| dir.join(n)).filter(|p| p.exists()).collect();
    if legacy.is_empty() {
        return Ok(());
    }
    let steam = crate::steam::SteamInstall::detect()?.dir;
    let accounts = crate::steam::accounts(&steam);
    let owner = owner_of_legacy(dir, &steam, &accounts).context("no Steam account to give the older files to")?;
    let target = dir.join("accounts").join(owner.to_string());
    std::fs::create_dir_all(&target)?;
    for from in legacy {
        let to = target.join(from.file_name().context("no file name")?);
        if !to.exists() {
            std::fs::rename(&from, &to).with_context(|| format!("moving {}", from.display()))?;
        }
    }
    Ok(())
}

/// Which account older files were made for: the one whose Steam collections they mention most.
/// Collection ids are random, so a match is no accident. Without any match, the account whose
/// folder Steam wrote to last, which is how older versions chose.
pub(crate) fn owner_of_legacy(dir: &Path, steam: &Path, accounts: &[crate::steam::Account]) -> Option<u32> {
    use crate::model::{Collection, Snapshot};
    #[derive(Deserialize, Default)]
    struct Draft {
        #[serde(default)]
        base: Vec<Collection>,
        #[serde(default)]
        draft: Vec<Collection>,
    }
    let read = |p: PathBuf| std::fs::read_to_string(p).ok();
    let mut mentioned: BTreeSet<String> = BTreeSet::new();
    if let Some(d) = read(dir.join("draft.json")).and_then(|t| serde_json::from_str::<Draft>(&t).ok()) {
        mentioned.extend(d.base.into_iter().chain(d.draft).map(|c| c.id));
    }
    if let Some(c) = read(dir.join("colors.json")).and_then(|t| serde_json::from_str::<BTreeMap<String, String>>(&t).ok()) {
        mentioned.extend(c.into_keys());
    }
    let newest_snapshot = std::fs::read_dir(dir.join("snapshots")).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir()).max();
    if let Some(s) = newest_snapshot.and_then(|p| read(p.join("collections.json"))).and_then(|t| serde_json::from_str::<Snapshot>(&t).ok()) {
        mentioned.extend(s.collections.into_iter().map(|c| c.id));
    }
    let score = |id: u32| {
        let install = crate::steam::SteamInstall { dir: steam.to_path_buf(), account_id: id };
        crate::catalog::Catalog::read(&install).map_or(0, |cat| cat.collections().iter().filter(|c| mentioned.contains(&c.id)).count())
    };
    let best = accounts.iter().map(|a| (score(a.id), a.id)).filter(|(s, _)| *s > 0).max_by_key(|(s, _)| *s);
    best.map(|(_, id)| id).or_else(|| accounts.iter().max_by_key(|a| a.touched).map(|a| a.id))
}

pub fn path() -> Result<PathBuf> {
    Ok(account_dir()?.join("config.toml"))
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
        let backups = account_dir()?.join("config-backups");
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
