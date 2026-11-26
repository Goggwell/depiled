//! Steam's local cache of store data for every app the client knows about
//! (`appcache/appinfo.vdf`): names, types, tags, store flags and review scores.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::vdf::{BinaryReader, Value};

const V27: u32 = 0x0756_4427;
const V28: u32 = 0x0756_4428;
const V29: u32 = 0x0756_4429;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Kind {
    Game,
    Demo,
    Dlc,
    Tool,
    Application,
    Music,
    Video,
    Beta,
    #[default]
    Other,
}

impl Kind {
    fn parse(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "game" => Kind::Game,
            "demo" => Kind::Demo,
            "dlc" => Kind::Dlc,
            "tool" => Kind::Tool,
            "application" => Kind::Application,
            "music" => Kind::Music,
            "video" | "series" | "episode" => Kind::Video,
            "beta" => Kind::Beta,
            _ => Kind::Other,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct AppInfo {
    pub appid: u32,
    pub name: String,
    pub kind: Kind,
    pub tags: Vec<u32>,
    pub categories: Vec<u32>,
    pub metacritic: u32,
    pub review_percent: u32,
    /// Unix seconds, 0 if unknown.
    pub release: u64,
    pub developers: Vec<String>,
    pub publishers: Vec<String>,
    /// Free to play, so it shows in the library once launched even without a license.
    pub free: bool,
    pub extras: Extras,
}

/// Store details used for filters. Zero means unknown.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Extras {
    /// Steam Deck: 1 unsupported, 2 playable, 3 verified.
    pub deck: u8,
    /// 1 partial, 2 full controller support.
    pub controller: u8,
    /// Bits: 1 Windows, 2 macOS, 4 Linux.
    pub platforms: u8,
    pub vr: bool,
    pub early_access: bool,
}

const TAG_EARLY_ACCESS: u32 = 493;
const TAG_VR: u32 = 21978;
/// Store categories for VR support, VR supported and VR only.
const VR_CATEGORIES: [u32; 3] = [31, 53, 54];

pub fn path(steam_dir: &Path) -> std::path::PathBuf {
    steam_dir.join("appcache").join("appinfo.vdf")
}

pub fn load(steam_dir: &Path) -> Result<HashMap<u32, AppInfo>> {
    let file = path(steam_dir);
    let data = std::fs::read(&file).with_context(|| format!("reading {}", file.display()))?;
    parse(&data)
}

pub fn parse(data: &[u8]) -> Result<HashMap<u32, AppInfo>> {
    let mut r = BinaryReader::new(data);
    let magic = r.u32()?;
    if ![V27, V28, V29].contains(&magic) {
        bail!("appinfo.vdf has an unknown format (0x{magic:08x})");
    }
    let _universe = r.u32()?;

    // Newer files keep every key once in a table at the end, and refer to it by index.
    let table: Option<Vec<String>> = if magic == V29 {
        let offset = r.u64()? as usize;
        let mut t = BinaryReader::new(data.get(offset..).context("string table offset out of range")?);
        let count = t.u32()? as usize;
        let mut strings = Vec::with_capacity(count);
        for _ in 0..count {
            strings.push(t.cstr()?);
        }
        Some(strings)
    } else {
        None
    };

    let mut apps = HashMap::new();
    loop {
        let appid = r.u32()?;
        if appid == 0 {
            break;
        }
        let size = r.u32()? as usize;
        let start = r.pos;
        let end = start.checked_add(size).filter(|&e| e <= data.len()).context("app entry runs past the end")?;
        // info state, last updated, access token, sha1, change number, binary sha1
        let header = 4 + 4 + 8 + 20 + 4 + if magic == V27 { 0 } else { 20 };
        let mut kv = BinaryReader { data: &data[start + header..end], pos: 0, strings: table.as_deref() };
        if let Ok(items) = kv.object() {
            let root = Value::Obj(items);
            if let Some(info) = extract(appid, &root) {
                apps.insert(appid, info);
            }
        }
        r.pos = end;
    }
    Ok(apps)
}

fn extract(appid: u32, root: &Value) -> Option<AppInfo> {
    let app = root.get("appinfo")?;
    let common = app.get("common")?;
    let num = |v: Option<&Value>| v.and_then(Value::as_i64).unwrap_or(0).max(0) as u64;

    let tags: Vec<u32> = common.get("store_tags").map(|t| t.entries().iter().filter_map(|(_, v)| v.as_i64()).map(|v| v as u32).collect()).unwrap_or_default();
    let categories: Vec<u32> = common
        .get("category")
        .map(|c| c.entries().iter().filter_map(|(k, _)| k.strip_prefix("category_")?.parse().ok()).collect())
        .unwrap_or_default();

    let mut developers = Vec::new();
    let mut publishers = Vec::new();
    for (_, a) in common.get("associations").map(Value::entries).unwrap_or_default() {
        let (Some(kind), Some(name)) = (a.get("type").and_then(Value::as_str), a.get("name").and_then(Value::as_str)) else { continue };
        match kind {
            "developer" => developers.push(name.to_owned()),
            "publisher" => publishers.push(name.to_owned()),
            _ => {}
        }
    }
    let os = common.get("oslist").and_then(Value::as_str).unwrap_or_default();
    let extras = Extras {
        deck: common.path(&["steam_deck_compatibility", "category"]).and_then(Value::as_i64).unwrap_or(0).clamp(0, 3) as u8,
        controller: match common.get("controller_support").and_then(Value::as_str) {
            Some("full") => 2,
            Some("partial") => 1,
            _ => 0,
        },
        platforms: [("windows", 1), ("macos", 2), ("linux", 4)].iter().filter(|(n, _)| os.contains(n)).map(|(_, b)| b).sum(),
        vr: tags.contains(&TAG_VR) || VR_CATEGORIES.iter().any(|c| categories.contains(c)),
        early_access: tags.contains(&TAG_EARLY_ACCESS),
    };
    let release = [common.get("steam_release_date"), common.get("original_release_date")].into_iter().map(num).find(|&t| t > 0).unwrap_or(0);

    Some(AppInfo {
        appid,
        name: common.get("name").and_then(Value::as_str).unwrap_or_default().to_owned(),
        kind: Kind::parse(common.get("type").and_then(Value::as_str).unwrap_or_default()),
        tags,
        categories,
        metacritic: num(common.get("metacritic_score")) as u32,
        review_percent: num(common.get("review_percentage")) as u32,
        release,
        developers,
        publishers,
        free: [app.path(&["extended", "isfreeapp"]), common.get("isfreeapp")].into_iter().any(|v| v.and_then(Value::as_i64) == Some(1)),
        extras,
    })
}
