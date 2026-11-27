//! Ready-made sets of collections for a way of playing: each collection fills itself from its
//! criteria and lets go of games that stop matching, so a game moves from "In progress" to
//! "Completed" by itself. Picking a preset creates the collections that are missing and gives
//! criteria to ones that already exist by the same name.

use serde::Serialize;
use serde_json::json;

use crate::smart::SmartRule;

#[derive(Debug, Clone, Serialize)]
pub struct Preset {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub collections: Vec<PresetCollection>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PresetCollection {
    pub name: &'static str,
    pub rule: SmartRule,
}

fn rule(condition: serde_json::Value) -> SmartRule {
    SmartRule { condition: serde_json::from_value(condition).ok(), sync: true, ..SmartRule::default() }
}

fn test(field: &str, op: &str, value: serde_json::Value) -> serde_json::Value {
    json!({ "field": field, "op": op, "value": value })
}

pub fn all() -> Vec<Preset> {
    let has = test("has_achievements", "is", json!(true));
    let completed = test("completed", "is", json!(true));
    let not_completed = test("completed", "is", json!(false));
    vec![
        Preset {
            id: "achievements",
            name: "Achievement hunting",
            description: "Games with achievements, by how far you've got. Achievement progress from the internet makes this most accurate.",
            collections: vec![
                PresetCollection { name: "Completed", rule: rule(json!({ "all": [completed] })) },
                PresetCollection {
                    name: "In progress",
                    rule: rule(json!({ "all": [has, test("achievements_unlocked", "gt", json!(0)), not_completed] })),
                },
                PresetCollection {
                    name: "Haven't started",
                    rule: rule(json!({ "all": [has, { "any": [test("achievements_unlocked", "eq", json!(0)), test("played", "is", json!("never"))] }] })),
                },
                PresetCollection {
                    name: "Quick to 100%",
                    rule: rule(json!({ "all": [has, not_completed, test("perfect_hours", "lte", json!(10))] })),
                },
            ],
        },
        Preset {
            id: "backlog",
            name: "Backlog",
            description: "What you're playing, what you've put down, and what you haven't touched.",
            collections: vec![
                PresetCollection { name: "Playing", rule: rule(json!({ "all": [test("last_played_days", "lt", json!(14))] })) },
                PresetCollection {
                    name: "On hold",
                    rule: rule(json!({ "all": [test("played", "is", json!("played")), test("last_played_days", "gte", json!(14)), not_completed] })),
                },
                PresetCollection { name: "Not played yet", rule: rule(json!({ "all": [test("played", "is", json!("never"))] })) },
            ],
        },
        Preset {
            id: "deck",
            name: "Steam Deck",
            description: "Valve's Steam Deck rating for each game.",
            collections: vec![
                PresetCollection { name: "Deck Verified", rule: rule(json!({ "all": [test("deck", "is", json!("verified"))] })) },
                PresetCollection { name: "Deck Playable", rule: rule(json!({ "all": [test("deck", "is", json!("playable"))] })) },
                PresetCollection { name: "Not for Deck", rule: rule(json!({ "all": [test("deck", "is", json!("unsupported"))] })) },
            ],
        },
        Preset {
            id: "linux",
            name: "Linux",
            description: "Games with a Linux version, and Windows games ProtonDB rates Gold or Platinum.",
            collections: vec![
                PresetCollection { name: "Native Linux", rule: rule(json!({ "all": [test("platforms", "has_any", json!(["linux"]))] })) },
                PresetCollection {
                    name: "Runs well on Proton",
                    rule: rule(json!({ "all": [test("platforms", "has_none", json!(["linux"])), test("proton", "in", json!(["platinum", "gold"]))] })),
                },
            ],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_preset_collection_has_valid_criteria() {
        for p in all() {
            for c in &p.collections {
                assert!(c.rule.condition.is_some(), "{} / {}", p.name, c.name);
                c.rule.to_rule(c.name).unwrap_or_else(|e| panic!("{} / {}: {e}", p.name, c.name));
                assert!(c.rule.sync, "preset collections let go of games");
            }
        }
    }
}
