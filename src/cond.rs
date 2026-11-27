//! Conditions with AND, OR and NOT for collections and filters, e.g. "publisher contains
//! Annapurna, and last played between 1 and 3 years ago", or "fewer than 100 achievements, over
//! 90% positive reviews, and costs under 30".
//!
//! The same JSON shape is evaluated by the window (ui/src/lib/criteria.ts) for instant counts,
//! and here when rules are applied. Unknown values (no price, never played) never match a
//! comparison; `known` and `unknown` test for them explicitly.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::Game;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Cond {
    All { all: Vec<Cond> },
    Any { any: Vec<Cond> },
    Not { not: Box<Cond> },
    Test { field: String, op: String, #[serde(default)] value: Value },
}

const DAY: u64 = 86_400;

/// Facts about one game that conditions can test, beyond the game itself.
pub struct Context<'a> {
    pub now: u64,
    /// Ids of the collections the game is in.
    pub collections: &'a [String],
}

/// One fact about a game, borrowed from it: evaluating a rule over a whole library copies
/// nothing.
enum Fact<'a> {
    Text(&'a [String]),
    Num(Option<f64>),
    Choice(&'a str),
    Bool(bool),
    /// Tag or feature ids.
    Ids(&'a [u32]),
    Names(Names<'a>),
}

enum Names<'a> {
    Owned(&'a [String]),
    Fixed(Vec<&'static str>),
}

impl Names<'_> {
    fn any(&self, f: impl Fn(&str) -> bool) -> bool {
        match self {
            Names::Owned(v) => v.iter().any(|s| f(s)),
            Names::Fixed(v) => v.iter().any(|s| f(s)),
        }
    }
}

fn fact<'a>(field: &str, g: &'a Game, ctx: &Context<'a>) -> Option<Fact<'a>> {
    let n = |v: f64| Fact::Num(Some(v));
    let known = |v: f64| Fact::Num((v > 0.0).then_some(v));
    let total = g.achievements.map_or(0, |a| a.total).max(g.online.achievements);
    Some(match field {
        "name" => Fact::Text(std::slice::from_ref(&g.name)),
        "developer" => Fact::Text(&g.developers),
        "publisher" => Fact::Text(&g.publishers),
        "playtime_hours" => n(f64::from(g.playtime_minutes) / 60.0),
        "last_played_days" => Fact::Num((g.last_played > 0).then(|| (ctx.now.saturating_sub(g.last_played) / DAY) as f64)),
        "release_year" => Fact::Num((g.release > 0).then(|| year(g.release) as f64)),
        "review_percent" => known(f64::from(g.review_percent)),
        "review_count" => known(g.online.review_count as f64),
        "metacritic" => known(f64::from(g.metacritic)),
        "achievements" => known(f64::from(total)),
        "achievements_unlocked" => Fact::Num(g.achievements.filter(|a| a.total > 0).map(|a| f64::from(a.unlocked))),
        "achievement_percent" => Fact::Num(g.achievements.filter(|a| a.total > 0).map(|a| f64::from(a.unlocked) * 100.0 / f64::from(a.total.max(total)))),
        "perfect_hours" => known(f64::from(g.online.perfect_minutes) / 60.0),
        "price" => Fact::Num(g.online.price_cents.map(|c| c as f64 / 100.0)),
        "discount" => n(f64::from(g.online.discount)),
        "size_gb" => known(g.installed_bytes as f64 / 1_073_741_824.0),
        "played" => Fact::Choice(if g.started() { "played" } else { "never" }),
        "deck" => Fact::Choice(["unknown", "unsupported", "playable", "verified"][usize::from(g.extras.deck.min(3))]),
        "proton" => Fact::Choice(if g.online.proton.is_empty() { "none" } else { &g.online.proton }),
        "installed" => Fact::Bool(g.installed_bytes > 0),
        "vr" => Fact::Bool(g.extras.vr),
        "early_access" => Fact::Bool(g.extras.early_access),
        "family" => Fact::Bool(g.family),
        "on_sale" => Fact::Bool(g.online.discount > 0),
        // Steam's own "Steam Achievements" feature, or a count from anywhere. Games without
        // either have none.
        "has_screenshots" => Fact::Bool(g.screenshots),
        "has_achievements" => Fact::Bool(total > 0 || g.categories.contains(&crate::rules::ACHIEVEMENTS_FEATURE)),
        "completed" => Fact::Bool(g.achievements.is_some_and(|a| a.total > 0 && a.unlocked >= a.total.max(total))),
        "tags" => Fact::Ids(&g.tags),
        "features" => Fact::Ids(&g.categories),
        "platforms" => Fact::Names(Names::Fixed(
            [("windows", 1u8), ("mac", 2), ("linux", 4)].into_iter().filter(|(_, b)| g.extras.platforms & b != 0).map(|(n, _)| n).collect(),
        )),
        "collections" => Fact::Names(Names::Owned(ctx.collections)),
        _ => return None,
    })
}

/// The calendar year (UTC) of a unix time, matching what the window computes.
fn year(secs: u64) -> i64 {
    // Days to civil date (Howard Hinnant's algorithm), keeping only the year.
    let z = (secs / DAY) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    yoe + era * 400 + i64::from(mp >= 10)
}

fn num(v: &Value) -> Option<f64> {
    v.as_f64().or_else(|| v.as_str()?.trim().parse().ok())
}

/// Whether a value the user picked names `have`: the same text ignoring case, or the same
/// number (written as a number or as text). Same rules as the window's `same`.
fn names(value: &Value, have: &str) -> bool {
    match value.as_str() {
        Some(v) => v.eq_ignore_ascii_case(have) || num(value).zip(have.trim().parse::<f64>().ok()).is_some_and(|(a, b)| a == b),
        None => num(value).zip(have.trim().parse::<f64>().ok()).is_some_and(|(a, b)| a == b),
    }
}

fn is_id(value: &Value, id: u32) -> bool {
    num(value).is_some_and(|v| v == f64::from(id))
}

/// The values a set test asks about: a list, or a single value.
fn wanted(value: &Value) -> &[Value] {
    value.as_array().map_or(std::slice::from_ref(value), Vec::as_slice)
}

fn test(field: &str, op: &str, value: &Value, g: &Game, ctx: &Context) -> bool {
    let Some(f) = fact(field, g, ctx) else { return false };
    match f {
        Fact::Text(items) => {
            let want = value.as_str().unwrap_or_default().trim().to_lowercase();
            let any = |p: &dyn Fn(&str) -> bool| items.iter().any(|s| p(&s.to_lowercase()));
            match op {
                "contains" => any(&|s| s.contains(&want)),
                "not_contains" => !any(&|s| s.contains(&want)),
                "is" => any(&|s| s == want),
                "is_not" => !any(&|s| s == want),
                "starts_with" => any(&|s| s.starts_with(&want)),
                _ => false,
            }
        }
        Fact::Num(n) => match (op, n) {
            ("known", n) => n.is_some(),
            ("unknown", n) => n.is_none(),
            (_, None) => false,
            ("between", Some(x)) => {
                let range = value.as_array().map(|a| (a.first().and_then(num), a.get(1).and_then(num)));
                range.is_some_and(|(lo, hi)| lo.is_none_or(|lo| x >= lo) && hi.is_none_or(|hi| x <= hi))
            }
            (op, Some(x)) => num(value).is_some_and(|v| match op {
                "lt" => x < v,
                "lte" => x <= v,
                "gt" => x > v,
                "gte" => x >= v,
                "eq" => (x - v).abs() < 1e-9,
                _ => false,
            }),
        },
        Fact::Choice(c) => match op {
            "is" => names(value, c),
            "is_not" => !names(value, c),
            "in" => value.as_array().is_some_and(|a| a.iter().any(|v| names(v, c))),
            _ => false,
        },
        Fact::Bool(b) => match op {
            // A missing or malformed value means "yes", as in the window.
            "is" => value.as_bool().unwrap_or(true) == b,
            _ => false,
        },
        Fact::Ids(have) => set_test(op, wanted(value), |w| have.iter().any(|&id| is_id(w, id))),
        Fact::Names(have) => set_test(op, wanted(value), |w| have.any(|h| names(w, h))),
    }
}

fn set_test(op: &str, wanted: &[Value], has: impl Fn(&Value) -> bool) -> bool {
    match op {
        "has_any" => wanted.iter().any(&has),
        "has_all" => wanted.iter().all(&has),
        "has_none" => !wanted.iter().any(&has),
        _ => false,
    }
}

impl Cond {
    pub fn matches(&self, g: &Game, ctx: &Context) -> bool {
        match self {
            // An empty group matches everything, like an empty filter.
            Cond::All { all } => all.iter().all(|c| c.matches(g, ctx)),
            Cond::Any { any } => any.is_empty() || any.iter().any(|c| c.matches(g, ctx)),
            Cond::Not { not } => !not.matches(g, ctx),
            Cond::Test { field, op, value } => test(field, op, value, g, ctx),
        }
    }

    /// True when the condition has no tests at all.
    pub fn is_empty(&self) -> bool {
        match self {
            Cond::All { all } => all.iter().all(Cond::is_empty),
            Cond::Any { any } => any.iter().all(Cond::is_empty),
            Cond::Not { not } => not.is_empty(),
            Cond::Test { .. } => false,
        }
    }

    /// Fields this condition uses that aren't known, so a typo is reported instead of silently
    /// matching nothing.
    pub fn unknown_fields(&self) -> Vec<String> {
        let probe = Game::default();
        let ctx = Context { now: 0, collections: &[] };
        let mut out = Vec::new();
        self.walk(&mut |field| {
            if fact(field, &probe, &ctx).is_none() {
                out.push(field.to_owned());
            }
        });
        out
    }

    fn walk(&self, f: &mut dyn FnMut(&str)) {
        match self {
            Cond::All { all: items } | Cond::Any { any: items } => items.iter().for_each(|c| c.walk(f)),
            Cond::Not { not } => not.walk(f),
            Cond::Test { field, .. } => f(field),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Achievements, Online};

    const NOW: u64 = 2_000_000_000;

    fn game() -> Game {
        Game {
            appid: 1,
            name: "Outer Wilds".into(),
            publishers: vec!["Annapurna Interactive".into()],
            developers: vec!["Mobius Digital".into()],
            playtime_minutes: 1_200,
            last_played: NOW - 400 * DAY,
            review_percent: 95,
            achievements: Some(Achievements { unlocked: 20, total: 31 }),
            tags: vec![21, 1664],
            online: Online { price_cents: Some(2_499), review_count: 80_000, ..Online::default() },
            ..Game::default()
        }
    }

    fn parse(json: &str) -> Cond {
        serde_json::from_str(json).unwrap()
    }

    fn check(json: &str) -> bool {
        let cols = vec!["uc-a".to_owned()];
        parse(json).matches(&game(), &Context { now: NOW, collections: &cols })
    }

    #[test]
    fn the_examples_from_the_brief() {
        assert!(check(r#"{"field":"publisher","op":"contains","value":"annapurna"}"#));
        assert!(check(r#"{"field":"last_played_days","op":"between","value":[365,1095]}"#));
        assert!(check(
            r#"{"all":[{"field":"achievements","op":"lt","value":100},{"field":"review_percent","op":"gt","value":90},{"field":"price","op":"lt","value":30}]}"#
        ));
        assert!(!check(r#"{"field":"price","op":"lt","value":20}"#));
    }

    #[test]
    fn groups_combine() {
        assert!(check(r#"{"any":[{"field":"name","op":"is","value":"Celeste"},{"field":"tags","op":"has_any","value":[1664]}]}"#));
        assert!(!check(r#"{"not":{"field":"tags","op":"has_all","value":[21,1664]}}"#));
        assert!(check(r#"{"all":[]}"#), "an empty group matches everything");
        assert!(check(r#"{"field":"collections","op":"has_any","value":["uc-a"]}"#));
        assert!(check(r#"{"field":"collections","op":"has_none","value":["uc-b"]}"#));
    }

    #[test]
    fn unknown_values_never_match_comparisons() {
        let mut g = game();
        g.last_played = 0;
        g.online.price_cents = None;
        let ctx = Context { now: NOW, collections: &[] };
        for json in [r#"{"field":"last_played_days","op":"gt","value":0}"#, r#"{"field":"price","op":"lt","value":1000}"#] {
            assert!(!parse(json).matches(&g, &ctx), "{json}");
        }
        assert!(parse(r#"{"field":"price","op":"unknown"}"#).matches(&g, &ctx));
        assert_eq!(parse(r#"{"field":"publisherr","op":"is","value":"x"}"#).unknown_fields(), vec!["publisherr"]);
    }

    #[test]
    fn values_compare_like_the_window() {
        // Ids written as text, choices in any case, and a bool test without a value.
        assert!(check(r#"{"field":"tags","op":"has_all","value":["21",1664]}"#));
        assert!(check(r#"{"field":"played","op":"is","value":"PLAYED"}"#));
        assert!(check(r#"{"field":"deck","op":"in","value":["unknown","verified"]}"#));
        assert!(!check(r#"{"field":"family","op":"is"}"#), "no value means yes, and this isn't a family game");
        assert!(check(r#"{"field":"family","op":"is","value":false}"#));
        assert!(check(r#"{"field":"platforms","op":"has_none","value":["linux"]}"#));
        assert!(!check(r#"{"field":"playtime_hours","op":"between","value":5}"#), "a range needs two ends");
        assert!(check(r#"{"field":"playtime_hours","op":"between","value":[null,20]}"#), "an open end is no limit");
    }

    #[test]
    fn years_are_exact_at_the_edges() {
        assert_eq!(year(1_704_067_199), 2023); // 2023-12-31 23:59:59 UTC
        assert_eq!(year(1_704_067_200), 2024); // 2024-01-01 00:00:00 UTC
        assert_eq!(year(951_782_400), 2000); // 2000-02-29
    }
}
