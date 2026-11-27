use serde::{Deserialize, Serialize};

/// A user collection as the app sees it, independent of where it was read from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub apps: Vec<u32>,
    /// Dynamic (rule-based) collections are recomputed by Steam, so we never edit them.
    #[serde(default)]
    pub dynamic: bool,
}

impl Collection {
    /// Only ordinary user collections are managed. Favorites and Hidden are built-in lists
    /// Steam edits through dedicated calls, and re-creating them would collide by name.
    pub fn is_user_collection_id(id: &str) -> bool {
        id.starts_with("uc-")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Achievements {
    pub unlocked: u32,
    pub total: u32,
}

/// What the rules can look at for one game. Zero means "unknown" for the optional numbers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    pub appid: u32,
    #[serde(default)]
    pub name: String,
    /// game, demo, playtest, software, tool, soundtrack, video, shortcut (a non-Steam game), or
    /// unknown (Steam has no details on disk).
    #[serde(default = "game_kind")]
    pub kind: String,
    #[serde(default)]
    pub playtime_minutes: u32,
    /// Unix seconds, 0 if never launched.
    #[serde(default)]
    pub last_played: u64,
    /// Steam store category ids (flags such as Single-player or Steam Cloud).
    #[serde(default)]
    pub categories: Vec<u32>,
    #[serde(default)]
    pub tags: Vec<u32>,
    #[serde(default)]
    pub metacritic: u32,
    #[serde(default)]
    pub review_percent: u32,
    #[serde(default)]
    pub achievements: Option<Achievements>,
    /// When the achievement progress was last read or written, 0 if unknown.
    #[serde(default)]
    pub achievements_at: u64,
    /// Unix seconds, 0 if unknown.
    #[serde(default)]
    pub release: u64,
    #[serde(default)]
    pub developers: Vec<String>,
    #[serde(default)]
    pub publishers: Vec<String>,
    #[serde(default)]
    pub extras: crate::appinfo::Extras,
    /// Bytes on disk, 0 if not installed.
    #[serde(default)]
    pub installed_bytes: u64,
    /// Lent by someone in the account's Steam Family rather than owned.
    #[serde(default)]
    pub family: bool,
    /// Steam's change number for the game's info (see `AppInfo::change`), 0 if unknown.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub change: u32,
    /// Details fetched online (when switched on), for rules and filters.
    #[serde(default)]
    pub online: Online,
}

/// What the online extras know about a game. Defaults mean unknown.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Online {
    /// In the store region's currency, in cents.
    pub price_cents: Option<u64>,
    pub discount: u32,
    pub review_count: u64,
    /// ProtonDB tier, empty if unknown.
    pub proton: String,
    /// Median minutes to unlock every achievement (SteamHunters).
    pub perfect_minutes: u32,
    /// Number of achievements according to SteamHunters.
    pub achievements: u32,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

fn game_kind() -> String {
    "game".to_owned()
}

impl Default for Game {
    fn default() -> Self {
        Self {
            appid: 0,
            name: String::new(),
            kind: game_kind(),
            playtime_minutes: 0,
            last_played: 0,
            categories: Vec::new(),
            tags: Vec::new(),
            metacritic: 0,
            review_percent: 0,
            achievements: None,
            achievements_at: 0,
            release: 0,
            developers: Vec::new(),
            publishers: Vec::new(),
            extras: crate::appinfo::Extras::default(),
            installed_bytes: 0,
            family: false,
            change: 0,
            online: Online::default(),
        }
    }
}

impl Game {
    /// Rules and auto-fill only look at Steam games the account owns: not demos, software or
    /// soundtracks, not non-Steam games (Steam doesn't record their playtime), and not games lent
    /// by the Steam Family, which come and go as the owner plays them.
    pub fn is_game(&self) -> bool {
        self.kind == "game" && !self.family
    }

    /// Launched at least once. Steam rounds playtime to whole minutes, so a very short
    /// session shows 0 minutes but still has a last-played time.
    pub fn started(&self) -> bool {
        self.playtime_minutes > 0 || self.last_played > 0
    }
}

/// Point-in-time copy of every collection, used for undo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub taken_at: u64,
    pub label: String,
    pub collections: Vec<Collection>,
}
