//! Completion statistics from SteamHunters (steamhunters.com): how long players take to unlock
//! every achievement, and how many have. Fetched in batches now and then and kept on disk, so
//! the app works offline with whatever was last downloaded.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::config;

const API: &str = "https://steamhunters.com/api/apps";
/// The API takes a list of ids; longer URLs are refused.
const BATCH: usize = 100;
/// Stats change slowly, so a game is looked up again only after this long. Games new to the
/// library have never been looked up, so they are fetched at the next check.
pub const REFRESH_AFTER_SECS: u64 = 7 * 24 * 3600;
const PAUSE: Duration = Duration::from_millis(1500);

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

/// Games whose stats are missing or older than `REFRESH_AFTER_SECS`.
pub fn stale(cache: &Cache, appids: &[u32], now: u64) -> Vec<u32> {
    appids.iter().copied().filter(|a| cache.checked.get(a).is_none_or(|&t| now.saturating_sub(t) >= REFRESH_AFTER_SECS)).collect()
}

/// Looks up the stale games in `appids` (all of them with `force`) in batches, saving after
/// each one so an interrupted refresh keeps what it got. `progress` is told how many are done.
pub fn refresh(appids: &[u32], now: u64, force: bool, progress: &mut dyn FnMut(usize, usize)) -> Result<usize> {
    let mut cache = load();
    let todo = if force { appids.to_vec() } else { stale(&cache, appids, now) };
    let agent = crate::online::agent();
    let mut done = 0;
    for (i, batch) in todo.chunks(BATCH).enumerate() {
        if i > 0 {
            std::thread::sleep(PAUSE);
        }
        crate::online::check_cancel()?;
        let ids = batch.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
        let mut response = agent.get(API).query("appIds", &ids).call().context("SteamHunters didn't answer")?;
        let sent = crate::online::sent_size(&response);
        let text = response.body_mut().read_to_string()?;
        crate::online::count_text(sent, &text);
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
    fn only_old_or_missing_games_are_fetched() {
        let mut cache = Cache::default();
        cache.checked.insert(1, 1_000);
        cache.checked.insert(2, 1_000 + REFRESH_AFTER_SECS);
        assert_eq!(stale(&cache, &[1, 2, 3], 1_000 + REFRESH_AFTER_SECS), vec![1, 3]);
    }

    #[test]
    fn parses_the_api_shape() {
        let text = r#"[{"appId":620,"name":"Portal 2","medianCompletionTime":2009,"fastestCompletionTime":152,"playersPerfectedCount":6863,"playerCount":95058,"achievementCount":51}]"#;
        let apps: Vec<ApiApp> = serde_json::from_str(text).unwrap();
        assert_eq!((apps[0].app_id, apps[0].median_completion_time, apps[0].players_perfected_count), (620, Some(2009), Some(6863)));
    }
}
