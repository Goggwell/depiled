//! Rough timings for large libraries. Not part of the normal test run:
//! `cargo test --release --test perf -- --ignored --nocapture`

use std::time::Instant;

use depiled::model::{Collection, Game};
use depiled::rules::{Rule, plan};
use depiled::service::membership;

const NOW: u64 = 1_800_000_000;

fn library(n: u32) -> (Vec<Game>, Vec<Collection>) {
    let games: Vec<Game> = (0..n)
        .map(|i| Game {
            appid: 1000 + i,
            name: format!("Game {i}"),
            playtime_minutes: if i % 3 == 0 { 0 } else { i % 5000 },
            last_played: if i % 3 == 0 { 0 } else { NOW - u64::from(i % 120) * 86_400 },
            tags: (0..12).map(|t| (i + t * 7) % 400).collect(),
            categories: vec![2, 22, 23],
            ..Game::default()
        })
        .collect();
    // 15 collections, the biggest holding about a third of the library.
    let collections = (0..15)
        .map(|c| Collection {
            id: format!("uc-{c}"),
            name: format!("Collection {c}"),
            apps: games.iter().filter(|g| (g.appid as usize + c).is_multiple_of(3 + c % 5)).map(|g| g.appid).collect(),
            dynamic: false,
        })
        .collect();
    (games, collections)
}

fn status_rules() -> Vec<Rule> {
    let mk = |name: &str, f: &dyn Fn(&mut Rule)| {
        let mut r = Rule { collection: name.into(), group: Some("status".into()), ..Rule::default() };
        f(&mut r);
        r
    };
    vec![
        mk("Unfinished", &|r| { r.played = Some(true); r.not_played_for_days = Some(7); }),
        mk("Started", &|r| r.played = Some(true)),
        mk("Planned", &|r| r.played = Some(false)),
    ]
}

/// What loading the library used to do: search every collection for every game.
fn naive_membership(games: &[Game], collections: &[Collection]) -> usize {
    games
        .iter()
        .map(|g| collections.iter().filter(|c| !c.dynamic && c.apps.contains(&g.appid)).count())
        .sum()
}

#[test]
#[ignore]
fn timings_for_large_libraries() {
    for n in [1_000, 10_000, 25_000] {
        let (games, collections) = library(n);

        let t = Instant::now();
        let old = if n <= 10_000 { Some(naive_membership(&games, &collections)) } else { None };
        let old_ms = t.elapsed().as_secs_f64() * 1000.0;

        let t = Instant::now();
        let map = membership(&collections);
        let new_ms = t.elapsed().as_secs_f64() * 1000.0;

        let t = Instant::now();
        let ops = plan(&status_rules(), &games, &collections, NOW);
        let plan_ms = t.elapsed().as_secs_f64() * 1000.0;

        println!(
            "{n:>6} games: membership old {} -> new {new_ms:.1} ms | rules plan {plan_ms:.1} ms ({} changes) | {} games in a collection{}",
            old.map_or("skipped (too slow)".into(), |_| format!("{old_ms:.0} ms")),
            ops.len(),
            map.len(),
            if let Some(o) = old { format!(" (check {o})") } else { String::new() },
        );
    }
}
