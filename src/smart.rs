//! "Smart" collections: a collection that remembers what belongs in it. The criteria mirror
//! the filters in the UI, are stored per collection name, and feed the same rules engine.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::config;
use crate::rules::Rule;

const IDLE_DAYS: u32 = 7;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
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
    /// Further conditions with AND, OR and NOT, as built in the app.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<crate::cond::Cond>,
    /// Also take games out when they stop matching, not just add new ones.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub sync: bool,
}

impl SmartRule {
    pub fn is_empty(&self) -> bool {
        self.played.is_none()
            && self.achievements.is_none()
            && self.min_review == 0
            && self.flags.is_empty()
            && self.tags.is_empty()
            && self.condition.as_ref().is_none_or(crate::cond::Cond::is_empty)
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
        rule.condition.clone_from(&self.condition);
        rule.sync = self.sync;
        rule.validate()?;
        Ok(rule)
    }
}

fn path() -> Result<PathBuf> {
    Ok(config::account_dir()?.join("smart-collections.json"))
}

/// Keyed by lower-cased collection name, like the rules file.
pub type Rules = BTreeMap<String, SmartRule>;

/// How a collection is looked up by name: ignoring case, and ignoring the order number
/// Depiled puts in front when the user keeps their own order ("03 Playing" is "playing").
pub fn key(name: &str) -> String {
    base_name(name).to_lowercase()
}

/// A collection's name without its order number.
pub fn base_name(name: &str) -> &str {
    let name = name.trim();
    order_number(name).map_or(name, |(_, rest)| rest)
}

/// The order number in front of a name and the rest of it: two or three digits and a space.
pub fn order_number(name: &str) -> Option<(u32, &str)> {
    let digits = name.bytes().take_while(u8::is_ascii_digit).count();
    let rest = name.get(digits..)?.strip_prefix(' ')?;
    ((2..=3).contains(&digits) && !rest.trim().is_empty()).then(|| (name[..digits].parse().ok(), rest.trim())).and_then(|(n, r)| Some((n?, r)))
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
    fn order_numbers_are_ignored_when_matching_names() {
        assert_eq!(key("03 Playing"), "playing");
        assert_eq!(key("Playing"), "playing");
        assert_eq!(base_name("100 Later"), "Later");
        // A name that merely starts with a number keeps it.
        assert_eq!(key("7 Days to Die"), "7 days to die");
        assert_eq!(key("2048"), "2048");
        assert_eq!(key("12 "), "12");
        assert_eq!(order_number("04 Co-op"), Some((4, "Co-op")));
    }

    #[test]
    fn criteria_map_onto_engine_rules() {
        let smart = SmartRule {
            played: Some("idle".into()),
            achievements: Some("incomplete".into()),
            min_review: 80,
            flags: vec![2, 23],
            tags: vec![1667],
            condition: None,
            sync: false,
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
