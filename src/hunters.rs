//! Completion statistics from SteamHunters (steamhunters.com): how long players take to unlock
//! every achievement, and how many have. Fetched in batches now and then and kept on disk, so
//! the app works offline with whatever was last downloaded.

use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::config;

const API: &str = "https://steamhunters.com/api/apps";
/// The API takes a list of ids; longer URLs are refused.
const BATCH: usize = 100;
const DAY: u64 = 24 * 3600;
/// Stats drift slowly as more people play, so an unchanged game is looked up again only after
/// this long (plus up to a week, spread by game). Games new to the library are fetched at the
/// next check.
pub const REFRESH_AFTER_SECS: u64 = 30 * DAY;
/// A game Steam reports as changed is looked up again once this much time has passed, since
/// an update can have added achievements. Busy games change several times a day; one look a
/// day is plenty.
const CHANGED_AFTER_SECS: u64 = DAY;
/// Least time between the starts of two requests.
const GAP_MS: u64 = 1500;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Stats {
    /// Minutes to unlock every achievement, for the median player. 0 if unknown.
    pub median_minutes: u32,
    pub fastest_minutes: u32,
    pub perfected: u32,
    pub players: u32,
    pub achievements: u32,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Cache {
    /// When each game was last looked up (also for games SteamHunters doesn't know).
    #[serde(default)]
    pub checked: BTreeMap<u32, u64>,
    #[serde(default)]
    pub stats: BTreeMap<u32, Stats>,
    /// Steam's change number for each game when it was looked up.
    #[serde(default)]
    pub change: BTreeMap<u32, u32>,
}

fn path() -> Result<PathBuf> {
    Ok(config::dir()?.join("steamhunters.json"))
}

pub fn load() -> Cache {
    path().ok().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn save(cache: &Cache) -> Result<()> {
    let path = path()?;
    std::fs::create_dir_all(path.parent().context("no parent")?)?;
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_vec(cache)?)?;
    std::fs::rename(tmp, path)?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiApp {
    app_id: u32,
    #[serde(default)]
    median_completion_time: Option<u32>,
    #[serde(default)]
    fastest_completion_time: Option<u32>,
    #[serde(default)]
    players_perfected_count: Option<u32>,
    #[serde(default)]
    player_count: Option<u32>,
    #[serde(default)]
    achievement_count: Option<u32>,
}

/// Games worth looking up, given as (app id, Steam's change number): never looked up, changed
/// in Steam since the last look, or not looked up for a month. Steam's own record of changes
/// stands in for an "updated since" question the API can't answer, so most of a library is
/// skipped without a request.
pub fn stale(cache: &Cache, games: &[(u32, u32)], now: u64) -> Vec<u32> {
    games
        .iter()
        .filter(|&&(appid, change)| {
            let Some(&at) = cache.checked.get(&appid) else { return true };
            let age = now.saturating_sub(at);
            let changed = change != 0 && cache.change.get(&appid) != Some(&change);
            (changed && age >= CHANGED_AFTER_SECS) || age >= REFRESH_AFTER_SECS + crate::online::spread(appid, 7)
        })
        .map(|&(appid, _)| appid)
        .collect()
}

/// Looks up the stale games in `appids` (all of them with `force`) in batches, saving after
/// each one so an interrupted refresh keeps what it got. `progress` is told how many are done.
pub fn refresh(games: &[(u32, u32)], now: u64, force: bool, progress: &mut dyn FnMut(usize, usize)) -> Result<usize> {
    let mut cache = load();
    let todo: Vec<u32> = if force { games.iter().map(|g| g.0).collect() } else { stale(&cache, games, now) };
    let changes: BTreeMap<u32, u32> = games.iter().copied().collect();
    let agent = crate::online::agent();
    let mut done = 0;
    let pace = crate::online::Pace::new(GAP_MS);
    for batch in todo.chunks(BATCH) {
        let ids = batch.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
        let mut response = pace.send("SteamHunters", || agent.get(API).query("appIds", &ids).call())?;
        crate::online::ok(&response, "SteamHunters")?;
        let text = crate::online::read_text(&mut response, 8 * 1024 * 1024)?;
        let apps: Vec<ApiApp> = serde_json::from_str(&text).context("SteamHunters sent something unexpected")?;
        for a in apps {
            cache.stats.insert(
                a.app_id,
                Stats {
                    median_minutes: a.median_completion_time.unwrap_or(0),
                    fastest_minutes: a.fastest_completion_time.unwrap_or(0),
                    perfected: a.players_perfected_count.unwrap_or(0),
                    players: a.player_count.unwrap_or(0),
                    achievements: a.achievement_count.unwrap_or(0),
                },
            );
        }
        for a in batch {
            cache.checked.insert(*a, now);
            if let Some(&c) = changes.get(a).filter(|&&c| c != 0) {
                cache.change.insert(*a, c);
            }
        }
        save(&cache)?;
        done += batch.len();
        progress(done, todo.len());
    }
    Ok(done)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn games_are_looked_up_again_when_steam_says_they_changed() {
        let mut cache = Cache::default();
        let then = 1_000_000;
        for a in [1, 2, 3, 4] {
            cache.checked.insert(a, then);
            cache.change.insert(a, 50);
        }
        let week = then + 7 * DAY;
        // 1 unchanged, 2 changed, 3 changed but looked up an hour ago, 4 unknown change, 5 new.
        cache.checked.insert(3, week - 3600);
        let games = [(1, 50), (2, 51), (3, 51), (4, 0), (5, 9)];
        assert_eq!(stale(&cache, &games, week), vec![2, 5]);
        // A month and a week later, everything is due whatever Steam says.
        assert_eq!(stale(&cache, &games, then + REFRESH_AFTER_SECS + 8 * DAY), vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn caches_from_before_change_numbers_are_refreshed_once() {
        let mut cache = Cache::default();
        cache.checked.insert(1, 1_000_000);
        assert_eq!(stale(&cache, &[(1, 7)], 1_000_000 + 2 * DAY), vec![1]);
    }

    #[test]
    fn parses_the_api_shape() {
        let text = r#"[{"appId":620,"name":"Portal 2","medianCompletionTime":2009,"fastestCompletionTime":152,"playersPerfectedCount":6863,"playerCount":95058,"achievementCount":51}]"#;
        let apps: Vec<ApiApp> = serde_json::from_str(text).unwrap();
        assert_eq!((apps[0].app_id, apps[0].median_completion_time, apps[0].players_perfected_count), (620, Some(2009), Some(6863)));
    }
}
