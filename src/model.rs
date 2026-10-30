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
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Game {
    pub appid: u32,
    #[serde(default)]
    pub name: String,
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
}

impl Game {
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
