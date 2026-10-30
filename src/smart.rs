//! "Smart" collections: a collection that remembers what belongs in it. The criteria mirror
//! the filters in the UI, are stored per collection name, and feed the same rules engine.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::config;
use crate::rules::Rule;

const IDLE_DAYS: u32 = 7;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SmartRule {
    /// "never", "played" or "idle" (started but not played for a week).
    #[serde(default)]
    pub played: Option<String>,
    /// "complete" or "incomplete".
    #[serde(default)]
    pub achievements: Option<String>,
    #[serde(default)]
    pub min_review: u32,
    #[serde(default)]
    pub flags: Vec<u32>,
    #[serde(default)]
    pub tags: Vec<u32>,
}

impl SmartRule {
    pub fn is_empty(&self) -> bool {
        self.played.is_none() && self.achievements.is_none() && self.min_review == 0 && self.flags.is_empty() && self.tags.is_empty()
    }

    pub fn to_rule(&self, collection: &str) -> Result<Rule> {
        let mut rule = Rule { collection: collection.to_owned(), ..Rule::default() };
        match self.played.as_deref() {
            None => {}
            Some("never") => rule.played = Some(false),
            Some("played") => rule.played = Some(true),
            Some("idle") => {
                rule.played = Some(true);
                rule.not_played_for_days = Some(IDLE_DAYS);
            }
            Some(other) => bail!("unknown play status '{other}'"),
        }
        match self.achievements.as_deref() {
            None => {}
            Some("complete") => rule.all_achievements = Some(true),
            Some("incomplete") => rule.all_achievements = Some(false),
            Some(other) => bail!("unknown achievements filter '{other}'"),
        }
        rule.min_review_percent = (self.min_review > 0).then_some(self.min_review);
        rule.has_flags = self.flags.iter().map(ToString::to_string).collect();
        rule.has_tags = self.tags.iter().map(ToString::to_string).collect();
        Ok(rule)
    }
}

fn path() -> Result<PathBuf> {
    Ok(config::dir()?.join("smart-collections.json"))
}

/// Keyed by lower-cased collection name, like the rules file.
pub type Rules = BTreeMap<String, SmartRule>;

pub fn key(name: &str) -> String {
    name.trim().to_lowercase()
}

pub fn load_from(path: &Path) -> Result<Rules> {
    match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).with_context(|| format!("{} is not valid", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Rules::new()),
        Err(e) => Err(e.into()),
    }
}

/// Writes next to the target first, so a crash never leaves a half-written file.
pub fn save_to(path: &Path, rules: &Rules) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(rules)?)?;
    std::fs::rename(tmp, path)?;
    Ok(())
}

pub fn load() -> Result<Rules> {
    load_from(&path()?)
}

pub fn save(rules: &Rules) -> Result<()> {
    save_to(&path()?, rules)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn criteria_map_onto_engine_rules() {
        let smart = SmartRule {
            played: Some("idle".into()),
            achievements: Some("incomplete".into()),
            min_review: 80,
            flags: vec![2, 23],
            tags: vec![1667],
        };
        let rule = smart.to_rule("Scary").unwrap();
        assert_eq!(rule.played, Some(true));
        assert_eq!(rule.not_played_for_days, Some(7));
        assert_eq!(rule.all_achievements, Some(false));
        assert_eq!(rule.min_review_percent, Some(80));
        assert_eq!(rule.has_flags, ["2", "23"]);
        assert_eq!(rule.has_tags, ["1667"]);
        rule.validate().unwrap();
    }

    #[test]
    fn unknown_values_are_rejected() {
        let bad = SmartRule { played: Some("sometimes".into()), ..SmartRule::default() };
        assert!(bad.to_rule("X").is_err());
    }

    #[test]
    fn empty_rule_is_detected() {
        assert!(SmartRule::default().is_empty());
        assert!(!SmartRule { min_review: 5, ..SmartRule::default() }.is_empty());
    }

    #[test]
    fn save_and_load_roundtrip_and_missing_file_is_empty() {
        let dir = std::env::temp_dir().join(format!("sc-smart-{}", crate::snapshot::now_secs()));
        let file = dir.join("smart.json");
        assert!(load_from(&file).unwrap().is_empty());
        let mut rules = Rules::new();
        rules.insert(key(" Horror Night "), SmartRule { tags: vec![1667], ..SmartRule::default() });
        save_to(&file, &rules).unwrap();
        assert_eq!(load_from(&file).unwrap(), rules);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
