use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use crate::model::{Collection, Game};
use crate::ops::Op;

/// One user-defined rule. Every condition that is set must hold for a game to match.
///
/// Rules sharing a `group` are exclusive: the first matching rule in file order wins and the
/// game is taken out of the group's other collections. Rules without a group only add games.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    /// Target collection, named by the user. Matched case-insensitively and created if missing.
    pub collection: String,
    pub group: Option<String>,
    /// Games already in this collection are never moved out by another rule in the group.
    #[serde(default)]
    pub sticky: bool,
    /// For rules without a group: also remove games that stop matching.
    #[serde(default)]
    pub sync: bool,

    pub played: Option<bool>,
    pub not_played_for_days: Option<u32>,
    pub played_within_days: Option<u32>,
    pub all_achievements: Option<bool>,
    pub min_playtime_hours: Option<f64>,
    pub max_playtime_hours: Option<f64>,
    pub min_review_percent: Option<u32>,
    pub min_metacritic: Option<u32>,
    /// Steam flags by name ("Single-player", "Steam Cloud") or numeric category id.
    #[serde(default)]
    pub has_flags: Vec<String>,
    /// Store tags or genres by name ("Horror", "Roguelike") or id. The game needs all of them.
    #[serde(default)]
    pub has_tags: Vec<String>,
    /// Same, but one match is enough.
    #[serde(default)]
    pub has_any_tags: Vec<String>,
    /// Skip games already in any of these collections (matched ignoring case). Use it to keep
    /// a rule away from games you've deliberately filed elsewhere.
    #[serde(default)]
    pub not_in_collections: Vec<String>,
    /// Any further conditions, with AND, OR and NOT. See cond.
    #[serde(default)]
    pub condition: Option<crate::cond::Cond>,
}

/// Replaces tag names with ids using Steam's own tag list. Fails on names Steam doesn't know,
/// so a typo is reported instead of silently matching nothing.
pub fn resolve_tags(rules: &mut [Rule], names: &BTreeMap<u32, String>) -> Result<()> {
    let by_name: BTreeMap<String, u32> = names.iter().map(|(id, n)| (n.to_lowercase(), *id)).collect();
    for rule in rules {
        for tag in rule.has_tags.iter_mut().chain(rule.has_any_tags.iter_mut()) {
            if tag.parse::<u32>().is_ok() {
                continue;
            }
            match by_name.get(&tag.trim().to_lowercase()) {
                Some(id) => *tag = id.to_string(),
                None => bail!("rule '{}': Steam has no tag or genre called '{tag}'", rule.collection),
            }
        }
    }
    Ok(())
}

/// Steam's store features ("categories"), by the names the store shows. Rules name them
/// ignoring case. Features not listed here are still kept on each game; they just have no
/// name to show.
pub const FLAGS: &[(&str, u32)] = &[
    ("Single-player", 2),
    ("Multi-player", 1),
    ("Co-op", 9),
    ("Online co-op", 38),
    ("Shared/split screen", 24),
    ("Shared/split screen co-op", 39),
    ("Online PvP", 36),
    ("Shared/split screen PvP", 37),
    ("PvP", 49),
    ("LAN co-op", 48),
    ("LAN PvP", 47),
    ("Cross-platform multiplayer", 27),
    ("MMO", 20),
    ("Remote Play Together", 44),
    ("Remote Play on TV", 43),
    ("Remote Play on tablet", 42),
    ("Remote Play on phone", 41),
    ("Full controller support", 28),
    ("Partial controller support", 18),
    ("Tracked controller support", 52),
    ("VR supported", 53),
    ("VR only", 54),
    ("Steam Achievements", 22),
    ("Steam Trading Cards", 29),
    ("Steam Cloud", 23),
    ("Steam Workshop", 30),
    ("Steam Leaderboards", 25),
    ("Stats", 15),
    ("Steam Timeline", 63),
    ("Family Sharing", 62),
    ("In-app purchases", 35),
    ("Includes level editor", 17),
    ("Captions available", 13),
    ("Commentary available", 14),
    ("HDR available", 61),
    ("Valve Anti-Cheat enabled", 8),
];

/// The Steam feature that means a game has achievements.
pub const ACHIEVEMENTS_FEATURE: u32 = 22;

pub fn flag_id(name: &str) -> Option<u32> {
    let name = name.trim();
    name.parse().ok().or_else(|| FLAGS.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, id)| *id))
}

impl Rule {
    pub fn validate(&self) -> Result<()> {
        if self.collection.trim().is_empty() {
            bail!("a rule has an empty collection name");
        }
        for flag in &self.has_flags {
            if flag_id(flag).is_none() {
                bail!("rule '{}': unknown flag '{flag}'", self.collection);
            }
        }
        if let Some(field) = self.condition.as_ref().and_then(|c| c.unknown_fields().into_iter().next()) {
            bail!("rule '{}': there's no filter called '{field}'", self.collection);
        }
        Ok(())
    }

    pub fn matches(&self, g: &Game, now: u64) -> bool {
        self.matches_in(g, now, &[])
    }

    /// Like matches, knowing which collections (by id) the game is in, for conditions that
    /// ask about collections.
    pub fn matches_in(&self, g: &Game, now: u64, collections: &[String]) -> bool {
        if let Some(c) = &self.condition && !c.matches(g, &crate::cond::Context { now, collections }) {
            return false;
        }
        let played = g.started();
        let idle_days = (g.last_played > 0).then(|| now.saturating_sub(g.last_played) / 86_400);
        let hours = f64::from(g.playtime_minutes) / 60.0;

        if let Some(want) = self.played && want != played {
            return false;
        }
        if let Some(n) = self.not_played_for_days && !idle_days.is_some_and(|d| d >= u64::from(n)) {
            return false;
        }
        if let Some(n) = self.played_within_days && !idle_days.is_some_and(|d| d <= u64::from(n)) {
            return false;
        }
        if let Some(want) = self.all_achievements {
            // Unknown progress, or a game without achievements, matches neither "complete"
            // nor "in progress".
            let state = g.achievements.filter(|a| a.total > 0).map(|a| a.unlocked == a.total);
            if state != Some(want) {
                return false;
            }
        }
        if self.min_playtime_hours.is_some_and(|m| hours < m) || self.max_playtime_hours.is_some_and(|m| hours > m) {
            return false;
        }
        if self.min_review_percent.is_some_and(|m| g.review_percent < m) || self.min_metacritic.is_some_and(|m| g.metacritic < m) {
            return false;
        }
        let has = |t: &String| t.parse::<u32>().is_ok_and(|id| g.tags.contains(&id));
        let flags_ok = self.has_flags.iter().all(|f| flag_id(f).is_some_and(|id| g.categories.contains(&id)));
        let any_ok = self.has_any_tags.is_empty() || self.has_any_tags.iter().any(has);
        flags_ok && any_ok && self.has_tags.iter().all(has)
    }
}

struct Want {
    name: String,
    add: BTreeSet<u32>,
    remove: BTreeSet<u32>,
}

/// Computes the minimal changes that make `current` follow the rules. Collections that no
/// rule mentions are never touched, and dynamic collections are never edited.
pub fn plan(rules: &[Rule], games: &[Game], current: &[Collection], now: u64) -> Vec<Op> {
    let by_name: BTreeMap<String, &Collection> = current
        .iter()
        .filter(|c| !c.dynamic && Collection::is_user_collection_id(&c.id))
        .map(|c| (crate::smart::key(&c.name), c))
        .collect();
    let member_sets: BTreeMap<&str, BTreeSet<u32>> =
        by_name.iter().map(|(n, c)| (n.as_str(), c.apps.iter().copied().collect())).collect();
    let members = |name: &str| -> BTreeSet<u32> { member_sets.get(crate::smart::key(name).as_str()).cloned().unwrap_or_default() };
    // Which collections (by id) each game is in, for conditions that ask.
    let mut filed_in: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    for c in current.iter().filter(|c| !c.dynamic) {
        for a in &c.apps {
            filed_in.entry(*a).or_default().push(c.id.clone());
        }
    }
    // A rule is skipped for games the user keeps in one of its excluded collections.
    let applies = |r: &Rule, g: &Game| {
        !r.not_in_collections.iter().any(|n| member_sets.get(crate::smart::key(n).as_str()).is_some_and(|s| s.contains(&g.appid)))
            && r.matches_in(g, now, filed_in.get(&g.appid).map_or(&[], Vec::as_slice))
    };

    let mut wants: BTreeMap<String, Want> = BTreeMap::new();
    let add = |wants: &mut BTreeMap<String, Want>, name: &str, app: u32, put: bool| {
        let w = wants
            .entry(crate::smart::key(name))
            .or_insert_with(|| Want { name: name.to_owned(), add: BTreeSet::new(), remove: BTreeSet::new() });
        if put { w.add.insert(app) } else { w.remove.insert(app) };
    };

    let mut groups: BTreeMap<&str, Vec<&Rule>> = BTreeMap::new();
    for rule in rules {
        if let Some(group) = &rule.group {
            groups.entry(group.as_str()).or_default().push(rule);
        }
    }

    for group_rules in groups.values() {
        let current_members: Vec<BTreeSet<u32>> = group_rules.iter().map(|r| members(&r.collection)).collect();
        for g in games {
            let held = |i: usize| current_members[i].contains(&g.appid);
            if group_rules.iter().enumerate().any(|(i, r)| r.sticky && held(i)) {
                continue;
            }
            let Some(winner) = group_rules.iter().position(|r| applies(r, g)) else { continue };
            for (i, r) in group_rules.iter().enumerate() {
                if i == winner {
                    add(&mut wants, &r.collection, g.appid, true);
                } else if held(i) {
                    add(&mut wants, &r.collection, g.appid, false);
                }
            }
        }
    }

    for rule in rules.iter().filter(|r| r.group.is_none()) {
        let held = members(&rule.collection);
        for g in games {
            if applies(rule, g) {
                add(&mut wants, &rule.collection, g.appid, true);
            } else if rule.sync && held.contains(&g.appid) {
                add(&mut wants, &rule.collection, g.appid, false);
            }
        }
    }

    let mut ops = Vec::new();
    for want in wants.values() {
        match by_name.get(&crate::smart::key(&want.name)) {
            None => {
                if !want.add.is_empty() {
                    ops.push(Op::Create { id: crate::catalog::new_id(), name: want.name.clone(), apps: want.add.iter().copied().collect() });
                }
            }
            Some(c) => {
                let held: BTreeSet<u32> = c.apps.iter().copied().collect();
                let to_add: Vec<u32> = want.add.difference(&held).copied().collect();
                let to_remove: Vec<u32> = want.remove.iter().filter(|a| held.contains(a) && !want.add.contains(a)).copied().collect();
                if !to_add.is_empty() {
                    ops.push(Op::Add { id: c.id.clone(), name: c.name.clone(), apps: to_add });
                }
                if !to_remove.is_empty() {
                    ops.push(Op::Remove { id: c.id.clone(), name: c.name.clone(), apps: to_remove });
                }
            }
        }
    }
    ops
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Achievements;

    const NOW: u64 = 100 * 86_400;
    const DAY: u64 = 86_400;

    fn game(appid: u32, minutes: u32, days_ago: Option<u64>) -> Game {
        Game { appid, playtime_minutes: minutes, last_played: days_ago.map_or(0, |d| NOW - d * DAY), ..Game::default() }
    }

    fn col(id: &str, name: &str, apps: &[u32]) -> Collection {
        Collection { id: format!("uc-{id}"), name: name.into(), apps: apps.to_vec(), dynamic: false }
    }

    fn status_rules(days: u32) -> Vec<Rule> {
        let mk = |name: &str, f: &dyn Fn(&mut Rule)| {
            let mut r = Rule { collection: name.into(), group: Some("status".into()), ..Rule::default() };
            f(&mut r);
            r
        };
        vec![
            mk("Completed", &|r| { r.all_achievements = Some(true); r.sticky = true; }),
            mk("Unfinished", &|r| { r.played = Some(true); r.not_played_for_days = Some(days); }),
            mk("Started", &|r| r.played = Some(true)),
        ]
    }

    fn cols() -> Vec<Collection> {
        vec![col("1", "COMPLETED", &[]), col("2", "UNFINISHED", &[]), col("3", "STARTED", &[])]
    }

    #[test]
    fn recent_play_goes_to_started_and_idle_play_to_unfinished() {
        let games = [game(1, 60, Some(1)), game(2, 60, Some(30)), game(3, 0, None)];
        let ops = plan(&status_rules(7), &games, &cols(), NOW);
        assert!(ops.contains(&Op::Add { id: "uc-3".into(), name: "STARTED".into(), apps: vec![1] }));
        assert!(ops.contains(&Op::Add { id: "uc-2".into(), name: "UNFINISHED".into(), apps: vec![2] }));
        assert_eq!(ops.len(), 2, "unplayed game must not be placed anywhere");
    }

    #[test]
    fn idle_threshold_is_configurable() {
        let games = [game(1, 60, Some(10))];
        let strict = plan(&status_rules(7), &games, &cols(), NOW);
        let lenient = plan(&status_rules(14), &games, &cols(), NOW);
        assert!(matches!(&strict[0], Op::Add { name, .. } if name == "UNFINISHED"));
        assert!(matches!(&lenient[0], Op::Add { name, .. } if name == "STARTED"));
    }

    #[test]
    fn game_moves_out_of_the_old_status_collection() {
        let current = vec![col("1", "COMPLETED", &[]), col("2", "UNFINISHED", &[]), col("3", "STARTED", &[1])];
        let ops = plan(&status_rules(7), &[game(1, 60, Some(30))], &current, NOW);
        assert!(ops.contains(&Op::Remove { id: "uc-3".into(), name: "STARTED".into(), apps: vec![1] }));
        assert!(ops.contains(&Op::Add { id: "uc-2".into(), name: "UNFINISHED".into(), apps: vec![1] }));
    }

    #[test]
    fn completed_requires_every_achievement_and_known_progress() {
        let mut done = game(1, 600, Some(1));
        done.achievements = Some(Achievements { unlocked: 10, total: 10 });
        let mut almost = game(2, 600, Some(1));
        almost.achievements = Some(Achievements { unlocked: 9, total: 10 });
        let unknown = game(3, 600, Some(1));
        let ops = plan(&status_rules(7), &[done, almost, unknown], &cols(), NOW);
        assert!(ops.contains(&Op::Add { id: "uc-1".into(), name: "COMPLETED".into(), apps: vec![1] }));
        assert!(ops.contains(&Op::Add { id: "uc-3".into(), name: "STARTED".into(), apps: vec![2, 3] }));
    }

    #[test]
    fn sticky_completed_games_are_not_moved_to_unfinished() {
        let current = vec![col("1", "COMPLETED", &[1]), col("2", "UNFINISHED", &[]), col("3", "STARTED", &[])];
        assert!(plan(&status_rules(7), &[game(1, 600, Some(50))], &current, NOW).is_empty());
    }

    #[test]
    fn names_are_user_defined_and_matched_case_insensitively() {
        let mut rules = status_rules(7);
        rules[1].collection = "Shelved".into();
        let current = vec![col("9", "shelved", &[])];
        let ops = plan(&rules, &[game(2, 60, Some(30))], &current, NOW);
        assert_eq!(ops, vec![Op::Add { id: "uc-9".into(), name: "shelved".into(), apps: vec![2] }]);
    }

    #[test]
    fn missing_collections_are_created_only_when_needed() {
        let ops = plan(&status_rules(7), &[game(1, 60, Some(1))], &[], NOW);
        assert!(matches!(&ops[..], [Op::Create { name, apps, .. }] if name == "Started" && apps == &vec![1]));
        assert!(plan(&status_rules(7), &[game(1, 0, None)], &[], NOW).is_empty());
    }

    #[test]
    fn consistent_state_produces_no_changes() {
        let current = vec![col("1", "COMPLETED", &[]), col("2", "UNFINISHED", &[]), col("3", "STARTED", &[1])];
        assert!(plan(&status_rules(7), &[game(1, 60, Some(1))], &current, NOW).is_empty());
    }

    #[test]
    fn dynamic_collections_are_never_targeted() {
        let mut dynamic = col("3", "STARTED", &[]);
        dynamic.dynamic = true;
        let ops = plan(&status_rules(7), &[game(1, 60, Some(1))], &[dynamic], NOW);
        assert!(matches!(&ops[0], Op::Create { .. }), "falls back to creating rather than editing it");
    }

    #[test]
    fn flag_and_score_rules_add_without_removing_by_default() {
        let rule = Rule {
            collection: "Solo gems".into(),
            has_flags: vec!["Single-player".into(), "Steam Cloud".into()],
            min_review_percent: Some(90),
            ..Rule::default()
        };
        let mut hit = game(1, 0, None);
        hit.categories = vec![2, 23];
        hit.review_percent = 95;
        let mut miss = game(2, 0, None);
        miss.categories = vec![2];
        miss.review_percent = 95;
        let current = vec![col("5", "Solo gems", &[2])];
        let ops = plan(std::slice::from_ref(&rule), &[hit.clone(), miss.clone()], &current, NOW);
        assert_eq!(ops, vec![Op::Add { id: "uc-5".into(), name: "Solo gems".into(), apps: vec![1] }]);

        let synced = Rule { sync: true, ..rule };
        let ops = plan(&[synced], &[hit, miss], &current, NOW);
        assert!(ops.contains(&Op::Remove { id: "uc-5".into(), name: "Solo gems".into(), apps: vec![2] }));
    }

    #[test]
    fn in_progress_needs_known_unfinished_achievements() {
        let rule = Rule { collection: "X".into(), all_achievements: Some(false), ..Rule::default() };
        let mut partial = game(1, 60, Some(1));
        partial.achievements = Some(Achievements { unlocked: 3, total: 10 });
        let mut none = game(2, 60, Some(1));
        none.achievements = Some(Achievements { unlocked: 0, total: 0 });
        let mut done = game(3, 60, Some(1));
        done.achievements = Some(Achievements { unlocked: 10, total: 10 });
        assert!(rule.matches(&partial, NOW));
        assert!(!rule.matches(&none, NOW));
        assert!(!rule.matches(&done, NOW));
        assert!(!rule.matches(&game(4, 60, Some(1)), NOW), "unknown progress never matches");
    }

    fn with_planned(days: u32) -> Vec<Rule> {
        let mut rules = status_rules(days);
        rules.push(Rule { collection: "Planned".into(), group: Some("status".into()), played: Some(false), ..Rule::default() });
        rules
    }

    #[test]
    fn never_started_goes_to_planned_and_unfinished_is_only_for_started_games() {
        let current = vec![col("4", "PLANNED", &[])];
        let games = [game(1, 0, None), game(2, 60, Some(30))];
        let ops = plan(&with_planned(7), &games, &current, NOW);
        assert!(ops.contains(&Op::Add { id: "uc-4".into(), name: "PLANNED".into(), apps: vec![1] }));
        assert!(ops.iter().any(|o| matches!(o, Op::Create { name, apps, .. } if name == "Unfinished" && apps == &vec![2])));
        assert!(!ops.iter().any(|o| matches!(o, Op::Create { name, apps, .. } if name == "Unfinished" && apps.contains(&1))));
    }

    #[test]
    fn game_launched_for_under_a_minute_still_counts_as_started() {
        let brief = game(1, 0, Some(30));
        assert!(brief.started());
        let ops = plan(&with_planned(7), &[brief], &[], NOW);
        assert!(matches!(&ops[..], [Op::Create { name, apps, .. }] if name == "Unfinished" && apps == &vec![1]));
    }

    #[test]
    fn a_game_leaves_planned_once_it_is_played() {
        let current = vec![col("4", "PLANNED", &[1]), col("3", "STARTED", &[])];
        let ops = plan(&with_planned(7), &[game(1, 30, Some(0))], &current, NOW);
        assert!(ops.contains(&Op::Remove { id: "uc-4".into(), name: "PLANNED".into(), apps: vec![1] }));
        assert!(ops.contains(&Op::Add { id: "uc-3".into(), name: "STARTED".into(), apps: vec![1] }));
    }

    #[test]
    fn games_in_excluded_collections_are_skipped_by_that_rule() {
        let mut rules = with_planned(7);
        rules[3].not_in_collections = vec!["won't play".into()];
        let current = vec![col("4", "PLANNED", &[]), col("8", "WON'T PLAY", &[1])];
        let ops = plan(&rules, &[game(1, 0, None), game(2, 0, None)], &current, NOW);
        assert_eq!(ops, vec![Op::Add { id: "uc-4".into(), name: "PLANNED".into(), apps: vec![2] }]);
    }

    #[test]
    fn tag_and_genre_names_resolve_to_ids_and_typos_fail() {
        let names: BTreeMap<u32, String> = [(1667, "Horror".to_string()), (1716, "Roguelike".to_string())].into();
        let mut rules = vec![Rule {
            collection: "Spooky".into(),
            has_tags: vec!["horror".into()],
            has_any_tags: vec!["Roguelike".into(), "999".into()],
            ..Rule::default()
        }];
        resolve_tags(&mut rules, &names).unwrap();
        assert_eq!(rules[0].has_tags, ["1667"]);
        assert_eq!(rules[0].has_any_tags, ["1716", "999"]);

        let mut hit = game(1, 0, None);
        hit.tags = vec![1667, 1716];
        let mut miss = game(2, 0, None);
        miss.tags = vec![1667];
        assert!(rules[0].matches(&hit, NOW));
        assert!(!rules[0].matches(&miss, NOW));

        let mut typo = vec![Rule { collection: "X".into(), has_tags: vec!["Horrorr".into()], ..Rule::default() }];
        assert!(resolve_tags(&mut typo, &names).is_err());
    }

    #[test]
    fn unknown_flag_names_fail_validation() {
        let rule = Rule { collection: "X".into(), has_flags: vec!["Nonexistent".into()], ..Rule::default() };
        assert!(rule.validate().is_err());
        assert_eq!(flag_id("steam cloud"), Some(23));
        assert_eq!(flag_id("23"), Some(23));
    }
}
