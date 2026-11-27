//! The library read straight from Steam's files, with no client connection: owned games,
//! store data, playtime and whatever achievement progress Steam has cached.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use anyhow::{Context, Result};

use crate::appinfo::{self, Kind};
use crate::licenses;
use crate::model::{Achievements, Game};
use crate::steam::SteamInstall;
use crate::vdf::{self, BinaryReader, Value};

#[derive(Debug, Clone, Copy, Default)]
pub struct Play {
    pub minutes: u32,
    pub last_played: u64,
}

/// Playtime and last launch per app, from `localconfig.vdf`.
pub fn playtime(install: &SteamInstall) -> Result<HashMap<u32, Play>> {
    let file = install.config_dir().join("localconfig.vdf");
    let text = vdf::read_text(&file).with_context(|| format!("reading {}", file.display()))?;
    let root = vdf::parse_text(&text)?;
    let apps = root.path(&["UserLocalConfigStore", "Software", "Valve", "Steam", "apps"]);
    Ok(apps
        .map(Value::entries)
        .unwrap_or_default()
        .iter()
        .filter_map(|(id, v)| {
            let num = |k| v.get(k).and_then(Value::as_i64).unwrap_or(0).max(0) as u64;
            // Installing a game can stamp a last-played time before it was ever launched; a
            // game that really ran also has a playtime entry (even 0 for a few seconds).
            let launched = v.get("Playtime").is_some();
            Some((id.parse().ok()?, Play { minutes: num("Playtime") as u32, last_played: if launched { num("LastPlayed") } else { 0 } }))
        })
        .collect())
}

/// Achievement progress Steam keeps on disk. Steam only caches it for games whose details
/// were shown or that were played on this computer, so many games have none.
/// Each game's progress with the time Steam wrote it (unix seconds).
pub fn achievements(install: &SteamInstall) -> BTreeMap<u32, (Achievements, u64)> {
    let mut out = BTreeMap::new();

    // The library's own cache: one JSON file per game, with ready-made counts.
    if let Ok(dir) = std::fs::read_dir(install.config_dir().join("librarycache")) {
        for entry in dir.filter_map(Result::ok) {
            let path = entry.path();
            let Some(appid) = path.file_stem().and_then(|s| s.to_str()?.parse::<u32>().ok()) else { continue };
            if let Some(a) = std::fs::read_to_string(&path).ok().and_then(|t| library_cache_counts(&t)) {
                out.insert(appid, (a, modified(&path)));
            }
        }
    }

    // The stats cache written when a game runs: unlocked bits against the game's schema.
    let stats = install.dir.join("appcache").join("stats");
    let prefix = format!("UserGameStats_{}_", install.account_id);
    if let Ok(dir) = std::fs::read_dir(&stats) {
        for entry in dir.filter_map(Result::ok) {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some(appid) = name.strip_prefix(&prefix).and_then(|s| s.strip_suffix(".bin")).and_then(|s| s.parse::<u32>().ok()) else { continue };
            if out.contains_key(&appid) {
                continue;
            }
            if let Some(a) = stats_counts(&entry.path(), &stats.join(format!("UserGameStatsSchema_{appid}.bin"))) {
                out.insert(appid, (a, modified(&entry.path())));
            }
        }
    }
    out.retain(|_, (a, _)| a.total > 0);
    out
}

fn modified(path: &Path) -> u64 {
    std::fs::metadata(path).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs())
}

fn library_cache_counts(text: &str) -> Option<Achievements> {
    let entries: Vec<(String, serde_json::Value)> = serde_json::from_str(text).ok()?;
    let (_, entry) = entries.into_iter().find(|(k, _)| k == "achievements")?;
    let data = entry.get("data")?;
    Some(Achievements { unlocked: data.get("nAchieved")?.as_u64()? as u32, total: data.get("nTotal")?.as_u64()? as u32 })
}

fn read_kv(path: &Path) -> Option<Value> {
    let data = std::fs::read(path).ok()?;
    BinaryReader::new(&data).object().ok().map(Value::Obj)
}

fn stats_counts(user: &Path, schema: &Path) -> Option<Achievements> {
    let schema = read_kv(schema)?;
    let user = read_kv(user)?;
    let stats = schema.entries().first()?.1.get("stats")?;
    let cache = user.get("cache")?;
    let (mut unlocked, mut total) = (0, 0);
    for (id, stat) in stats.entries() {
        let kind = stat.get("type").map(|t| t.as_str().map(str::to_owned).unwrap_or_else(|| t.as_i64().unwrap_or(0).to_string()));
        if !matches!(kind.as_deref(), Some("ACHIEVEMENTS" | "4")) {
            continue;
        }
        let bits: Vec<i64> = stat.get("bits").map(Value::entries).unwrap_or_default().iter().filter_map(|(_, b)| b.get("bit")?.as_i64()).collect();
        let data = cache.get(id).and_then(|c| c.get("data")).and_then(Value::as_i64).unwrap_or(0);
        total += bits.len() as u32;
        unlocked += bits.iter().filter(|&&b| (0..64).contains(&b) && data & (1 << b) != 0).count() as u32;
    }
    Some(Achievements { unlocked, total })
}

/// What sort of library entry something is. Rules and auto-fill only ever look at games.
pub fn kind_name(kind: &Kind) -> Option<&'static str> {
    Some(match kind {
        Kind::Game => "game",
        Kind::Demo => "demo",
        Kind::Application => "software",
        Kind::Tool => "tool",
        Kind::Music => "soundtrack",
        Kind::Video => "video",
        Kind::Beta => "playtest",
        Kind::Dlc | Kind::Other => return None,
    })
}

/// Everything the library shows: owned games, demos, software and soundtracks, free games that
/// have been launched, anything filed in a collection (`in_collections`), and non-Steam games
/// added by hand. Tools only appear when they are in a collection, as Steam keeps many hidden
/// ones (redistributables and such) on every account.
pub fn library(install: &SteamInstall, in_collections: &BTreeSet<u32>) -> Result<Vec<Game>> {
    let info = appinfo::load(&install.dir)?;
    let licenses::Ownership { owned, family } = licenses::ownership(&install.dir, install.account_id)?;
    let play = playtime(install).unwrap_or_default();
    let progress = achievements(install);
    let sizes = installed(install);
    let shots = screenshots(install);
    let family_games = family.iter().filter(|id| info.get(id).is_some_and(|a| a.kind == Kind::Game)).copied();

    let played_free = play.iter().filter(|(id, p)| (p.minutes > 0 || p.last_played > 0) && info.get(id).is_some_and(|a| a.free)).map(|(id, _)| *id);
    let mut ids: Vec<u32> = owned.iter().copied().chain(played_free).chain(family_games).chain(in_collections.iter().copied()).collect();
    ids.sort_unstable();
    ids.dedup();

    let mut out: Vec<Game> = ids
        .into_iter()
        .filter_map(|appid| {
            let Some(app) = info.get(&appid).filter(|a| !a.name.is_empty()) else {
                // Filed in a collection, but Steam has nothing on it (often a delisted app).
                // Shown anyway, so the collection's count adds up and it can be taken out.
                return in_collections.contains(&appid).then(|| Game { appid, name: format!("Unknown app {appid}"), kind: "unknown".to_owned(), ..Game::default() });
            };
            let kind = kind_name(&app.kind)?;
            if kind == "tool" && !in_collections.contains(&appid) {
                return None;
            }
            let p = play.get(&appid).copied().unwrap_or_default();
            Some(Game {
                appid,
                name: app.name.clone(),
                kind: kind.to_owned(),
                playtime_minutes: p.minutes,
                last_played: p.last_played,
                categories: app.categories.clone(),
                tags: app.tags.clone(),
                metacritic: app.metacritic,
                review_percent: app.review_percent,
                achievements: progress.get(&appid).map(|(a, _)| *a),
                achievements_at: progress.get(&appid).map_or(0, |(_, t)| *t),
                release: app.release,
                developers: app.developers.clone(),
                publishers: app.publishers.clone(),
                extras: app.extras,
                installed_bytes: sizes.get(&appid).copied().unwrap_or(0),
                family: family.contains(&appid) && !owned.contains(&appid),
                change: app.change,
                screenshots: shots.contains(&appid),
                online: crate::model::Online::default(),
            })
        })
        .collect();
    out.extend(shortcuts(install).unwrap_or_default());
    Ok(out)
}

/// Games with screenshots Steam saved on this computer for the account. Steam lists them in
/// `userdata/<account>/760/screenshots.vdf` and keeps the files under `760/remote/<app>`.
pub fn screenshots(install: &SteamInstall) -> std::collections::HashSet<u32> {
    let base = install.dir.join("userdata").join(install.account_id.to_string()).join("760");
    let mut out = std::collections::HashSet::new();
    if let Some(root) = vdf::read_text(&base.join("screenshots.vdf")).ok().and_then(|t| vdf::parse_text(&t).ok()) {
        for (id, shots) in root.get("Screenshots").map(Value::entries).unwrap_or_default() {
            if let Ok(id) = id.parse::<u32>()
                && !shots.entries().is_empty()
            {
                out.insert(id);
            }
        }
    }
    for entry in std::fs::read_dir(base.join("remote")).into_iter().flatten().flatten() {
        if let Ok(id) = entry.file_name().to_string_lossy().parse::<u32>()
            && std::fs::read_dir(entry.path().join("screenshots")).is_ok_and(|mut d| d.any(|f| f.is_ok_and(|f| f.path().is_file())))
        {
            out.insert(id);
        }
    }
    out
}

/// Installed apps and their size on disk, from every Steam library folder.
pub fn installed(install: &SteamInstall) -> HashMap<u32, u64> {
    let file = install.dir.join("steamapps").join("libraryfolders.vdf");
    let Some(root) = vdf::read_text(&file).ok().and_then(|t| vdf::parse_text(&t).ok()) else { return HashMap::new() };
    let mut out = HashMap::new();
    for (_, folder) in root.get("libraryfolders").map(Value::entries).unwrap_or_default() {
        for (id, size) in folder.get("apps").map(Value::entries).unwrap_or_default() {
            if let Ok(id) = id.parse::<u32>() {
                // A size of 0 means the app is listed but not (fully) installed.
                let bytes = size.as_i64().unwrap_or(0).max(0) as u64;
                if bytes > 0 {
                    out.insert(id, bytes);
                }
            }
        }
    }
    out
}

/// Non-Steam games the user added to Steam by hand (`userdata/<id>/config/shortcuts.vdf`).
pub fn shortcuts(install: &SteamInstall) -> Result<Vec<Game>> {
    let file = install.config_dir().join("shortcuts.vdf");
    let data = match std::fs::read(&file) {
        Ok(d) => d,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).with_context(|| format!("reading {}", file.display())),
    };
    Ok(parse_shortcuts(&data))
}

fn parse_shortcuts(data: &[u8]) -> Vec<Game> {
    let Ok(items) = BinaryReader::new(data).object() else { return Vec::new() };
    let root = Value::Obj(items);
    root.get("shortcuts")
        .map(Value::entries)
        .unwrap_or_default()
        .iter()
        .filter_map(|(_, s)| {
            // Stored as a signed number; collections use the same bits unsigned.
            let appid = s.get("appid")?.as_i64()? as i32 as u32;
            let name = s.get("AppName").or_else(|| s.get("appname"))?.as_str()?.trim().to_owned();
            if name.is_empty() {
                return None;
            }
            Some(Game {
                appid,
                name,
                kind: "shortcut".to_owned(),
                last_played: s.get("LastPlayTime").and_then(Value::as_i64).unwrap_or(0).max(0) as u64,
                ..Game::default()
            })
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_library_cache_counts() {
        let text = r#"[["badge",{"version":1}],["achievements",{"version":2,"data":{"nTotal":18,"nAchieved":5}}]]"#;
        assert_eq!(library_cache_counts(text), Some(Achievements { unlocked: 5, total: 18 }));
        assert_eq!(library_cache_counts("[]"), None);
    }

    #[test]
    fn reads_non_steam_shortcuts() {
        let mut data = vec![0x00];
        data.extend(b"shortcuts\0");
        data.push(0x00);
        data.extend(b"0\0");
        data.push(0x02);
        data.extend(b"appid\0");
        data.extend((-1_234_567_890i32).to_le_bytes());
        data.push(0x01);
        data.extend(b"AppName\0My Emulator\0");
        data.push(0x02);
        data.extend(b"LastPlayTime\0");
        data.extend(1_700_000_000i32.to_le_bytes());
        data.extend([0x08, 0x08, 0x08]);
        let games = parse_shortcuts(&data);
        assert_eq!(games.len(), 1);
        assert_eq!((games[0].appid, games[0].name.as_str(), games[0].kind.as_str()), (-1_234_567_890i32 as u32, "My Emulator", "shortcut"));
        assert_eq!(games[0].last_played, 1_700_000_000);
    }
}
