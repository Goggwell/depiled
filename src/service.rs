//! High-level operations shared by the CLI and the desktop app.
//!
//! Everything works on a draft: edits change the app's own copy of the collections at once,
//! and Save writes them all to Steam's files in one go, after a preview, while Steam is closed.
//! Every save takes a snapshot first, so it can be undone.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::catalog::{self, Catalog};
use crate::hunters::{self, Stats};
use crate::model::{Achievements, Collection, Game};
use crate::ops::{self, Op};
use crate::steam::{self, SteamInstall};
use crate::{config, local, online, rules, smart, snapshot};

/// The flag file older versions created to reach Steam through its debug port.
const LEGACY_FLAG: &str = ".cef-enable-remote-debugging";
const UNDO_LIMIT: usize = 100;

#[derive(Debug, Serialize)]
pub struct Status {
    pub steam_found: bool,
    pub steam_running: bool,
    /// Steam's debug port is switched on, left over from an older version.
    pub legacy_debug_port: bool,
}

pub fn status() -> Status {
    let install = SteamInstall::detect().ok();
    Status {
        steam_found: install.is_some(),
        steam_running: steam::steam_is_running().unwrap_or(false),
        legacy_debug_port: install.is_some_and(|i| i.dir.join(LEGACY_FLAG).exists()),
    }
}

/// Removes the flag file that switched on Steam's debug port. Takes effect when Steam restarts.
pub fn disable_debug_port() -> Result<()> {
    let path = SteamInstall::detect()?.dir.join(LEGACY_FLAG);
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

/// Asks Steam to exit and waits until it has.
pub fn close_steam(timeout: Duration) -> Result<()> {
    if !steam::steam_is_running()? {
        return Ok(());
    }
    steam::shutdown()?;
    let until = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < until {
        std::thread::sleep(Duration::from_millis(500));
        if !steam::steam_is_running()? {
            // Give it a moment to finish writing its files.
            std::thread::sleep(Duration::from_secs(1));
            return Ok(());
        }
    }
    bail!("Steam is still running. Close it from its tray icon and try again.")
}

pub fn start_steam() -> Result<()> {
    steam::start(&SteamInstall::detect()?)
}

#[derive(Debug, Serialize)]
pub struct GameView {
    pub appid: u32,
    pub name: String,
    pub kind: String,
    pub playtime_minutes: u32,
    pub last_played: u64,
    pub categories: Vec<u32>,
    pub tags: Vec<u32>,
    pub metacritic: u32,
    pub review_percent: u32,
    pub release: u64,
    pub developers: Vec<String>,
    pub publishers: Vec<String>,
    pub achievements: Option<Achievements>,
    pub hunters: Option<Stats>,
    pub extras: crate::appinfo::Extras,
    pub installed_bytes: u64,
    pub family: bool,
    /// Prices and review counts from the store, for the chosen region.
    pub store: Option<StoreView>,
    /// ProtonDB tier: platinum, gold, silver, bronze or borked.
    pub proton: Option<String>,
    pub cover: Option<String>,
    /// Ids of the editable collections this game is in.
    pub collections: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct CollectionView {
    pub id: String,
    pub name: String,
    pub count: usize,
    pub dynamic: bool,
    pub color: Option<String>,
}

/// The part of the store data the window shows. The cache keeps more (names, release dates,
/// when the price was last checked) that the window either has already or doesn't need.
#[derive(Debug, Clone, Serialize)]
pub struct StoreView {
    pub price_cents: Option<u64>,
    pub price_text: Option<String>,
    pub discount: u32,
    pub discount_ends: u64,
    pub review_count: u64,
    pub review_label: String,
    pub fetched: u64,
}

impl From<&online::StoreInfo> for StoreView {
    fn from(s: &online::StoreInfo) -> Self {
        Self {
            price_cents: s.price_cents,
            price_text: s.price_text.clone(),
            discount: s.discount,
            discount_ends: s.discount_ends,
            review_count: s.review_count,
            review_label: s.review_label.clone(),
            fetched: s.fetched,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct FlagView {
    pub id: u32,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct Library {
    pub games: Vec<GameView>,
    pub collections: Vec<CollectionView>,
    pub tags: BTreeMap<u32, String>,
    pub flags: Vec<FlagView>,
    pub draft: DraftState,
    pub online: OnlineState,
}

#[derive(Debug, Serialize)]
pub struct OnlineState {
    /// Country the cached prices are for.
    pub country: String,
    /// This computer's region, used when the user hasn't picked one.
    pub detected_region: Option<String>,
    pub profile_private: bool,
    pub achievements_read: usize,
    pub usage: online::Usage,
    /// Regions with prices kept from before, so switching to them is instant.
    pub cached_regions: Vec<String>,
    /// Each source: when it last fetched and how many games it knows about.
    pub sources: Vec<SourceState>,
}

#[derive(Debug, Serialize)]
pub struct SourceState {
    /// Matches the setting that switches it, e.g. online_prices.
    pub key: &'static str,
    pub label: &'static str,
    pub updated: u64,
    pub items: usize,
}

#[derive(Debug, Serialize)]
pub struct DraftState {
    /// Number of changes waiting to be saved to Steam.
    pub pending: usize,
    /// Collections with unsaved changes.
    pub changed: Vec<String>,
    pub undo: Option<String>,
    pub redo: Option<String>,
    pub steam_running: bool,
    /// Steam changed its collections since they were loaded, and the draft was moved on top.
    pub rebased: bool,
}

#[derive(Debug, Serialize)]
pub struct OpView {
    pub kind: &'static str,
    pub collection: String,
    pub count: usize,
    /// First few game names, for the preview.
    pub sample: Vec<String>,
}

pub fn describe(ops: &[Op], games: &[Game]) -> Vec<OpView> {
    let names: HashMap<u32, &str> = games.iter().map(|g| (g.appid, g.name.as_str())).collect();
    ops.iter()
        .map(|op| {
            let (kind, collection, apps): (&'static str, &str, &[u32]) = match op {
                Op::Create { name, apps, .. } => ("create", name, apps),
                Op::Add { name, apps, .. } => ("add", name, apps),
                Op::Remove { name, apps, .. } => ("remove", name, apps),
                Op::Rename { to, .. } => ("rename", to, &[]),
                Op::Delete { name, .. } => ("delete", name, &[]),
            };
            OpView {
                kind,
                collection: collection.to_owned(),
                count: apps.len(),
                sample: apps.iter().take(8).filter_map(|a| names.get(a)).map(|n| (*n).to_owned()).collect(),
            }
        })
        .collect()
}

/// Which editable collections each game is in, in one pass.
pub fn membership(collections: &[Collection]) -> HashMap<u32, Vec<String>> {
    let mut map: HashMap<u32, Vec<String>> = HashMap::new();
    for c in collections.iter().filter(|c| !c.dynamic) {
        for app in &c.apps {
            map.entry(*app).or_default().push(c.id.clone());
        }
    }
    map
}

/// Steam's cached artwork: portrait capsule, then header, in the app's library cache folder.
pub fn cover_path(steam_dir: &Path, appid: u32) -> Option<String> {
    let dir = steam_dir.join("appcache").join("librarycache").join(appid.to_string());
    let direct = ["library_600x900.jpg", "header.jpg"].into_iter().map(|f| dir.join(f)).find(|p| p.exists());
    let nested = || {
        std::fs::read_dir(&dir).ok()?.filter_map(Result::ok).filter(|e| e.path().is_dir()).find_map(|e| {
            ["library_600x900.jpg", "library_header.jpg"].into_iter().map(|f| e.path().join(f)).find(|p| p.exists())
        })
    };
    direct.or_else(nested).map(|p| p.to_string_lossy().replace('\\', "/"))
}

/// Steam's tag names, bundled so they work offline.
pub fn tag_names() -> BTreeMap<u32, String> {
    #[derive(Deserialize)]
    struct Tag {
        tagid: u32,
        name: String,
    }
    serde_json::from_str::<Vec<Tag>>(include_str!("data/tags.json")).map(|t| t.into_iter().map(|t| (t.tagid, t.name)).collect()).unwrap_or_default()
}

fn colors_path() -> Result<PathBuf> {
    Ok(config::dir()?.join("colors.json"))
}

fn load_colors() -> BTreeMap<String, String> {
    colors_path().ok().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn save_colors(colors: &BTreeMap<String, String>) -> Result<()> {
    let path = colors_path()?;
    std::fs::create_dir_all(path.parent().context("no parent")?)?;
    std::fs::write(path, serde_json::to_vec_pretty(colors)?)?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Step {
    label: String,
    collections: Vec<Collection>,
}

/// Unsaved work kept on disk, so closing the app doesn't lose it.
#[derive(Debug, Default, Serialize, Deserialize)]
struct SavedDraft {
    base: Vec<Collection>,
    draft: Vec<Collection>,
}

fn draft_path() -> Result<PathBuf> {
    Ok(config::dir()?.join("draft.json"))
}

/// Replays the changes between `old_base` and `draft` on top of `current`. Used when Steam's
/// collections changed underneath unsaved work, so neither side is lost.
pub fn rebase(old_base: &[Collection], draft: &[Collection], current: &[Collection]) -> Vec<Collection> {
    let mut out = current.to_vec();
    for op in ops::plan_restore_exact(old_base, draft) {
        ops::apply(&mut out, &op);
    }
    out
}

pub struct Session {
    pub install: SteamInstall,
    pub games: Vec<Game>,
    pub tags: BTreeMap<u32, String>,
    hunters: BTreeMap<u32, Stats>,
    store: online::StoreCache,
    proton: BTreeMap<u32, online::Proton>,
    /// The profile's game details aren't public, so achievements can't be read online.
    profile_private: bool,
    /// When Steam last wrote each game's achievement progress to disk on this computer.
    local_progress_at: HashMap<u32, u64>,
    /// Cover art found on disk, by game. Finding it takes a folder listing per game, so hits are
    /// kept; games without art are looked for again on the next reload.
    covers: HashMap<u32, String>,
    /// For the sources panel, so showing it doesn't re-read the caches.
    achievements_checked: u64,
    achievements_read: usize,
    hunters_checked: u64,
    /// What Steam's files held when last read or saved.
    base: Vec<Collection>,
    /// What the user wants.
    draft: Vec<Collection>,
    undo: Vec<Step>,
    redo: Vec<Step>,
    rebased: bool,
}

#[derive(Debug, Serialize)]
pub struct SaveReport {
    pub snapshot: Option<String>,
    pub changes: usize,
    pub verified: bool,
}

impl Session {
    pub fn open() -> Result<Self> {
        let install = SteamInstall::detect().context("Steam isn't installed, or hasn't been signed in to on this computer")?;
        let mut s = Self {
            install,
            games: Vec::new(),
            tags: tag_names(),
            hunters: BTreeMap::new(),
            store: online::StoreCache::default(),
            proton: BTreeMap::new(),
            profile_private: false,
            local_progress_at: HashMap::new(),
            covers: HashMap::new(),
            achievements_checked: 0,
            achievements_read: 0,
            hunters_checked: 0,
            base: Vec::new(),
            draft: Vec::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            rebased: false,
        };
        // Older versions kept a copy of the Steam wishlist; the app no longer reads it.
        if let Ok(dir) = config::dir() {
            let _ = std::fs::remove_file(dir.join("wishlist.json"));
        }
        s.reload()?;
        Ok(s)
    }

    /// Re-reads the library and Steam's collections. Unsaved changes are kept and moved on top
    /// of anything Steam changed meanwhile.
    pub fn reload(&mut self) -> Result<()> {
        let current = Catalog::read(&self.install)?.collections();
        let filed: BTreeSet<u32> = current.iter().flat_map(|c| c.apps.iter().copied()).collect();
        self.games = local::library(&self.install, &filed)?;
        self.local_progress_at = self.games.iter().filter(|g| g.achievements_at > 0).map(|g| (g.appid, g.achievements_at)).collect();
        for g in &self.games {
            if !self.covers.contains_key(&g.appid)
                && let Some(path) = cover_path(&self.install.dir, g.appid)
            {
                self.covers.insert(g.appid, path);
            }
        }
        self.overlay_online();

        let saved: Option<SavedDraft> = draft_path().ok().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|t| serde_json::from_str(&t).ok());
        let (old_base, draft) = match saved {
            Some(s) if self.base.is_empty() => (s.base, s.draft),
            _ => (self.base.clone(), self.draft.clone()),
        };
        if old_base.is_empty() && draft.is_empty() {
            self.base = current.clone();
            self.draft = current;
        } else {
            if !same(&old_base, &current) && !same(&old_base, &draft) {
                self.rebased = true;
            }
            self.draft = rebase(&old_base, &draft, &current);
            self.base = current;
        }
        self.persist();
        Ok(())
    }

    /// Re-reads only what the online extras fetched, after a refresh. Steam's own files haven't
    /// changed, so they aren't read again.
    pub fn reload_online(&mut self) {
        self.overlay_online();
    }

    /// Lays the cached online data over the games read from Steam's files.
    fn overlay_online(&mut self) {
        let hunted = hunters::load();
        let profile: online::AchievementCache = online::load(online::ACHIEVEMENTS_FILE);
        for g in &mut self.games {
            // Whichever is newer wins: the profile page, or what Steam wrote to disk after a
            // play session or after the game's page was opened in the library.
            if let Some(p) = profile.games.get(&g.appid).filter(|p| p.total > 0 && p.fetched >= g.achievements_at) {
                g.achievements = Some(Achievements { unlocked: p.unlocked, total: p.total });
                g.achievements_at = p.fetched;
            }
            // A game update can add achievements after the last look. SteamHunters, checked
            // weekly, notices, so a "100%" game shows as incomplete until it is looked at again.
            if let (Some(a), Some(h)) = (g.achievements.as_mut(), hunted.stats.get(&g.appid)) {
                let looked = g.achievements_at;
                if h.achievements > a.total && hunted.checked.get(&g.appid).copied().unwrap_or(0) > looked {
                    a.total = h.achievements;
                }
            }
        }
        self.hunters_checked = hunted.checked.values().copied().max().unwrap_or(0);
        self.hunters = hunted.stats;
        self.profile_private = profile.private;
        self.achievements_checked = profile.checked;
        self.achievements_read = profile.games.values().filter(|p| p.total > 0).count();
        self.store = online::load(online::STORE_FILE);
        self.proton = online::load(online::PROTON_FILE);
        for g in &mut self.games {
            let store = self.store.apps.get(&g.appid).filter(|s| s.fetched > 0);
            let hunt = self.hunters.get(&g.appid);
            g.online = crate::model::Online {
                price_cents: store.and_then(|s| s.price_cents),
                discount: store.map_or(0, |s| s.discount),
                review_count: store.map_or(0, |s| s.review_count),
                proton: self.proton.get(&g.appid).map(|p| p.tier.clone()).unwrap_or_default(),
                perfect_minutes: hunt.map_or(0, |h| h.median_minutes),
                achievements: hunt.map_or(0, |h| h.achievements),
            };
        }
    }

    fn persist(&self) {
        let Ok(path) = draft_path() else { return };
        if self.pending_ops().is_empty() {
            let _ = std::fs::remove_file(path);
        } else if let Ok(bytes) = serde_json::to_vec(&SavedDraft { base: self.base.clone(), draft: self.draft.clone() }) {
            let _ = std::fs::create_dir_all(path.parent().unwrap_or(Path::new(".")));
            let _ = std::fs::write(path, bytes);
        }
    }

    pub fn collections(&self) -> &[Collection] {
        &self.draft
    }

    pub fn pending_ops(&self) -> Vec<Op> {
        ops::plan_restore_exact(&self.base, &self.draft)
    }

    pub fn pending(&self) -> Vec<OpView> {
        describe(&self.pending_ops(), &self.games)
    }

    pub fn collection_views(&self) -> Vec<CollectionView> {
        let colors = load_colors();
        self.draft.iter().map(|c| CollectionView { id: c.id.clone(), name: c.name.clone(), count: c.apps.len(), dynamic: c.dynamic, color: colors.get(&c.id).cloned() }).collect()
    }

    pub fn library(&self) -> Library {
        let mut member_of = membership(&self.draft);
        let sources = self.source_states();
        let achievements_read = sources.iter().find(|s| s.key == "online_achievements").map_or(0, |s| s.items);
        let games = self
            .games
            .iter()
            .map(|g| GameView {
                cover: self.covers.get(&g.appid).cloned(),
                collections: member_of.remove(&g.appid).unwrap_or_default(),
                appid: g.appid,
                name: g.name.clone(),
                kind: g.kind.clone(),
                playtime_minutes: g.playtime_minutes,
                last_played: g.last_played,
                categories: g.categories.clone(),
                tags: g.tags.clone(),
                metacritic: g.metacritic,
                review_percent: g.review_percent,
                release: g.release,
                developers: g.developers.clone(),
                publishers: g.publishers.clone(),
                achievements: g.achievements,
                hunters: self.hunters.get(&g.appid).copied(),
                extras: g.extras,
                installed_bytes: g.installed_bytes,
                family: g.family,
                store: self.store.apps.get(&g.appid).filter(|s| s.fetched > 0 && !s.name.is_empty()).map(StoreView::from),
                proton: self.proton.get(&g.appid).map(|p| p.tier.clone()).filter(|t| !t.is_empty()),
            })
            .collect();
        Library {
            games,
            collections: self.collection_views(),
            tags: self.tags.clone(),
            flags: rules::FLAGS.iter().map(|(n, id)| FlagView { id: *id, name: (*n).to_owned() }).collect(),
            draft: self.draft_state(),
            online: OnlineState {
                country: self.store.country.clone(),
                detected_region: steam::home_region(),
                profile_private: self.profile_private,
                achievements_read,
                usage: online::load(online::USAGE_FILE),
                cached_regions: std::iter::once(self.store.country.clone()).chain(self.store.others.keys().cloned()).filter(|c| !c.is_empty()).collect(),
                sources,
            },
        }
    }

    fn source_states(&self) -> Vec<SourceState> {
        let newest = |it: &mut dyn Iterator<Item = u64>| it.max().unwrap_or(0);
        vec![
            SourceState { key: "online_achievements", label: "Achievement progress", updated: self.achievements_checked, items: self.achievements_read },
            SourceState {
                key: "online_prices",
                label: "Prices and reviews",
                updated: newest(&mut self.store.apps.values().map(|s| s.fetched)),
                items: self.store.apps.values().filter(|s| !s.name.is_empty()).count(),
            },
            SourceState { key: "online_hunters", label: "Completion times", updated: self.hunters_checked, items: self.hunters.values().filter(|s| s.median_minutes > 0).count() },
            SourceState { key: "online_protondb", label: "ProtonDB ratings", updated: newest(&mut self.proton.values().map(|p| p.fetched)), items: self.proton.values().filter(|p| !p.tier.is_empty()).count() },
        ]
    }

    pub fn draft_state(&self) -> DraftState {
        let ops = self.pending_ops();
        let mut changed: Vec<String> = ops
            .iter()
            .filter_map(|op| match op {
                Op::Create { id, .. } | Op::Add { id, .. } | Op::Remove { id, .. } | Op::Rename { id, .. } => Some(id.clone()),
                Op::Delete { .. } => None,
            })
            .collect();
        changed.dedup();
        DraftState {
            pending: ops.len(),
            changed,
            undo: self.undo.last().map(|s| s.label.clone()),
            redo: self.redo.last().map(|s| s.label.clone()),
            steam_running: steam::steam_is_running().unwrap_or(false),
            rebased: self.rebased,
        }
    }

    /// Runs one edit on the draft as a single undo step. Edits that change nothing leave no step.
    fn edit(&mut self, label: &str, f: impl FnOnce(&mut Vec<Collection>) -> Result<()>) -> Result<()> {
        let before = self.draft.clone();
        f(&mut self.draft)?;
        if same(&before, &self.draft) {
            return Ok(());
        }
        self.undo.push(Step { label: label.to_owned(), collections: before });
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.rebased = false;
        self.persist();
        Ok(())
    }

    pub fn undo(&mut self) -> Option<String> {
        let step = self.undo.pop()?;
        let now = std::mem::replace(&mut self.draft, step.collections);
        self.redo.push(Step { label: step.label.clone(), collections: now });
        self.persist();
        Some(step.label)
    }

    pub fn redo(&mut self) -> Option<String> {
        let step = self.redo.pop()?;
        let now = std::mem::replace(&mut self.draft, step.collections);
        self.undo.push(Step { label: step.label.clone(), collections: now });
        self.persist();
        Some(step.label)
    }

    /// Throws away every unsaved change.
    pub fn discard(&mut self) {
        let before = std::mem::replace(&mut self.draft, self.base.clone());
        if !same(&before, &self.draft) {
            self.undo.push(Step { label: "discard changes".into(), collections: before });
        }
        self.persist();
    }

    fn find(&self, id: &str) -> Result<&Collection> {
        let c = self.draft.iter().find(|c| c.id == id).with_context(|| format!("collection {id} not found"))?;
        if c.dynamic {
            bail!("'{}' is a dynamic collection managed by Steam, so it can't be edited here", c.name);
        }
        Ok(c)
    }

    fn check_name(&self, name: &str, except: Option<&str>) -> Result<String> {
        let name = name.trim();
        if name.is_empty() {
            bail!("give the collection a name");
        }
        if self.draft.iter().any(|c| c.name.eq_ignore_ascii_case(name) && Some(c.id.as_str()) != except) {
            bail!("a collection called '{name}' already exists");
        }
        Ok(name.to_owned())
    }

    pub fn change_membership(&mut self, appids: &[u32], add_to: &[String], remove_from: &[String], label: &str) -> Result<()> {
        for id in add_to.iter().chain(remove_from) {
            self.find(id)?;
        }
        self.edit(label, |cols| {
            for c in cols.iter_mut() {
                if remove_from.contains(&c.id) {
                    c.apps.retain(|a| !appids.contains(a));
                }
                if add_to.contains(&c.id) {
                    let have: BTreeSet<u32> = c.apps.iter().copied().collect();
                    c.apps.extend(appids.iter().filter(|a| !have.contains(a)));
                }
            }
            Ok(())
        })
    }

    /// Returns the new collection's id.
    pub fn create_collection(&mut self, name: &str, appids: Vec<u32>) -> Result<String> {
        let name = self.check_name(name, None)?;
        let id = catalog::new_id();
        let c = Collection { id: id.clone(), name: name.clone(), apps: appids, dynamic: false };
        self.edit(&format!("create {name}"), |cols| {
            cols.push(c);
            Ok(())
        })?;
        Ok(id)
    }

    pub fn rename_collection(&mut self, id: &str, name: &str) -> Result<()> {
        let from = self.find(id)?.name.clone();
        let name = self.check_name(name, Some(id))?;
        self.edit(&format!("rename {from}"), |cols| {
            if let Some(c) = cols.iter_mut().find(|c| c.id == id) {
                c.name.clone_from(&name);
            }
            Ok(())
        })?;
        // Keep any auto-fill criteria attached to the renamed collection.
        let mut all = smart::load()?;
        if let Some(rule) = all.remove(&smart::key(&from)) {
            all.insert(smart::key(&name), rule);
            smart::save(&all)?;
        }
        Ok(())
    }

    pub fn delete_collection(&mut self, id: &str) -> Result<()> {
        let name = self.find(id)?.name.clone();
        self.edit(&format!("delete {name}"), |cols| {
            cols.retain(|c| c.id != id);
            Ok(())
        })
    }

    /// A new collection with the same games. Returns its id.
    pub fn duplicate_collection(&mut self, id: &str) -> Result<String> {
        let source = self.find(id)?.clone();
        let mut name = format!("{} copy", source.name);
        let mut n = 2;
        while self.draft.iter().any(|c| c.name.eq_ignore_ascii_case(&name)) {
            name = format!("{} copy {n}", source.name);
            n += 1;
        }
        let new = self.create_collection(&name, source.apps)?;
        if let Some(color) = load_colors().get(id).cloned() {
            self.set_color(&new, Some(color))?;
        }
        Ok(new)
    }

    /// Moves every game from `sources` into `into` and deletes the sources, as one step.
    pub fn merge_collections(&mut self, sources: &[String], into: &str) -> Result<()> {
        let target = self.find(into)?.name.clone();
        for s in sources {
            self.find(s)?;
        }
        let sources: Vec<String> = sources.iter().filter(|s| *s != into).cloned().collect();
        if sources.is_empty() {
            bail!("pick at least one other collection to merge");
        }
        self.edit(&format!("merge into {target}"), |cols| {
            let moved: Vec<u32> = cols.iter().filter(|c| sources.contains(&c.id)).flat_map(|c| c.apps.clone()).collect();
            if let Some(t) = cols.iter_mut().find(|c| c.id == into) {
                let have: BTreeSet<u32> = t.apps.iter().copied().collect();
                let mut seen = BTreeSet::new();
                t.apps.extend(moved.into_iter().filter(|a| !have.contains(a) && seen.insert(*a)));
            }
            cols.retain(|c| !sources.contains(&c.id));
            Ok(())
        })
    }

    /// Colors live in the app only; Steam has no such thing.
    pub fn set_color(&mut self, id: &str, color: Option<String>) -> Result<()> {
        self.find(id)?;
        let mut colors = load_colors();
        match color.filter(|c| c.starts_with('#') && c.len() <= 9) {
            Some(c) => colors.insert(id.to_owned(), c),
            None => colors.remove(id),
        };
        save_colors(&colors)
    }

    /// Makes the draft match an earlier snapshot exactly. Like any edit, it is saved with Save.
    pub fn restore_snapshot(&mut self, id: &str) -> Result<()> {
        let target = snapshot_collections(id)?;
        self.restore_to("restore an earlier version", &target, true)
    }

    /// Makes the draft match `target`. Without `exact`, collections that aren't in `target`
    /// are left alone, which suits restoring a backup from another device.
    pub fn restore_to(&mut self, label: &str, target: &[Collection], exact: bool) -> Result<()> {
        self.edit(label, |cols| {
            let diff = if exact { ops::plan_restore_exact(cols, target) } else { ops::plan_restore(cols, target) };
            for op in diff {
                ops::apply(cols, &op);
            }
            Ok(())
        })
    }

    fn games_with_rules(&self, rules: &mut [rules::Rule]) -> Result<(Vec<Game>, Vec<String>)> {
        rules::resolve_tags(rules, &self.tags)?;
        let mut notes = Vec::new();
        if rules.iter().any(|r| r.all_achievements.is_some()) {
            let unknown = self.games.iter().filter(|g| g.is_game() && g.started() && g.achievements.is_none()).count();
            if unknown > 0 {
                notes.push(format!("Achievement progress isn't known yet for {unknown} played games, so they can't count as completed."));
            }
        }
        Ok((self.games.iter().filter(|g| g.is_game()).cloned().collect(), notes))
    }

    /// Everything the user's rules ask for: the config file's rules plus each smart collection's.
    /// Criteria for collections that no longer exist are skipped, so deleting a collection
    /// isn't undone by the next Auto-organize.
    pub fn plan_rules(&self) -> Result<(Vec<Op>, Vec<String>)> {
        let (config, _) = config::load()?;
        let mut all = config.rules;
        let existing: BTreeSet<String> = self.draft.iter().filter(|c| !c.dynamic).map(|c| smart::key(&c.name)).collect();
        for (name, rule) in smart::load()?.into_iter().filter(|(n, _)| existing.contains(n)) {
            all.push(rule.to_rule(&name)?);
        }
        let (games, mut notes) = self.games_with_rules(&mut all)?;
        // A sticky "every achievement" collection keeps games that an update has since given
        // new achievements; point them out so the user can decide.
        for rule in all.iter().filter(|r| r.all_achievements == Some(true) && r.sticky) {
            let Some(col) = self.draft.iter().find(|c| c.name.eq_ignore_ascii_case(&rule.collection)) else { continue };
            let behind: Vec<&str> = games
                .iter()
                .filter(|g| col.apps.contains(&g.appid) && g.achievements.is_some_and(|a| a.total > 0 && a.unlocked < a.total))
                .map(|g| g.name.as_str())
                .collect();
            if !behind.is_empty() {
                let more = if behind.len() > 5 { format!(", and {} more", behind.len() - 5) } else { String::new() };
                notes.push(format!(
                    "{} in {} {} at 100% (often an update added achievements): {}{more}. They stay there; take them out if you like.",
                    behind.len(),
                    col.name,
                    if behind.len() == 1 { "isn't" } else { "aren't" },
                    behind[..behind.len().min(5)].join(", ")
                ));
            }
        }
        Ok((rules::plan(&all, &games, &self.draft, snapshot::now_secs()), notes))
    }

    pub fn preview_rules(&self) -> Result<(Vec<OpView>, Vec<String>)> {
        let (ops, notes) = self.plan_rules()?;
        Ok((describe(&ops, &self.games), notes))
    }

    fn apply_ops(&mut self, label: &str, ops: Vec<Op>) -> Result<usize> {
        let n = ops.len();
        self.edit(label, |cols| {
            for op in &ops {
                ops::apply(cols, op);
            }
            Ok(())
        })?;
        Ok(n)
    }

    /// Puts the rules' changes into the draft. Returns how many changes that was.
    pub fn apply_rules(&mut self) -> Result<usize> {
        let (ops, _) = self.plan_rules()?;
        self.apply_ops("auto-organize", ops)
    }

    fn smart_plan(&self, name: &str, smart: &smart::SmartRule) -> Result<(Vec<Op>, usize)> {
        let mut rule = vec![smart.to_rule(name)?];
        let (games, _) = self.games_with_rules(&mut rule)?;
        let now = snapshot::now_secs();
        // Count with each game's collections, as the plan does, so conditions on collections
        // give the same number in the preview as in the result.
        let filed_in = membership(&self.draft);
        let total = games.iter().filter(|g| rule[0].matches_in(g, now, filed_in.get(&g.appid).map_or(&[], Vec::as_slice))).count();
        Ok((rules::plan(&rule, &games, &self.draft, now), total))
    }

    /// Creates a collection filled with every game matching `smart`. With `auto`, the criteria
    /// are remembered so the collection can be topped up later. Returns (id, games added).
    pub fn create_smart_collection(&mut self, name: &str, smart: &smart::SmartRule, auto: bool) -> Result<(String, usize)> {
        if smart.is_empty() {
            bail!("pick at least one filter first");
        }
        let name = self.check_name(name, None)?;
        let (ops, _) = self.smart_plan(&name, smart)?;
        let apps: Vec<u32> = ops.iter().flat_map(|op| match op {
            Op::Create { apps, .. } | Op::Add { apps, .. } => apps.clone(),
            _ => Vec::new(),
        }).collect();
        let added = apps.len();
        let id = self.create_collection(&name, apps)?;
        if auto {
            let mut all = smart::load()?;
            all.insert(smart::key(&name), smart.clone());
            smart::save(&all)?;
        }
        Ok((id, added))
    }

    /// Smart rules by collection id, for the collections that currently exist.
    pub fn smart_rules(&self) -> Result<BTreeMap<String, smart::SmartRule>> {
        let stored = smart::load()?;
        Ok(self.draft.iter().filter(|c| !c.dynamic).filter_map(|c| stored.get(&smart::key(&c.name)).map(|r| (c.id.clone(), r.clone()))).collect())
    }

    pub fn set_smart_rule(&self, id: &str, rule: Option<smart::SmartRule>) -> Result<()> {
        let name = self.find(id)?.name.clone();
        let mut all = smart::load()?;
        match rule {
            Some(r) if !r.is_empty() => {
                r.to_rule(&name)?;
                all.insert(smart::key(&name), r);
            }
            Some(_) => bail!("pick at least one filter first"),
            None => {
                all.remove(&smart::key(&name));
            }
        }
        smart::save(&all)
    }

    /// Adds every game that fits the collection's saved criteria. Returns how many were added.
    pub fn fill_collection(&mut self, id: &str) -> Result<usize> {
        let name = self.find(id)?.name.clone();
        let smart = smart::load()?.remove(&smart::key(&name)).with_context(|| format!("'{name}' has no auto-fill criteria yet"))?;
        let (ops, _) = self.smart_plan(&name, &smart)?;
        let added = ops.iter().map(|op| if let Op::Add { apps, .. } = op { apps.len() } else { 0 }).sum();
        self.apply_ops(&format!("fill {name}"), ops)?;
        Ok(added)
    }

    /// (games matching in total, games not in the collection yet). Read-only.
    pub fn preview_smart(&self, smart: &smart::SmartRule, collection: Option<&str>) -> Result<(usize, usize)> {
        let name = match collection {
            Some(id) => self.find(id)?.name.clone(),
            None => "(preview)".to_owned(),
        };
        let (ops, total) = self.smart_plan(&name, smart)?;
        let new = ops.iter().map(|op| match op {
            Op::Create { apps, .. } | Op::Add { apps, .. } => apps.len(),
            _ => 0,
        }).sum();
        Ok((total, new))
    }

    /// Writes the draft to Steam's files: snapshot first, then the changes, then a check that
    /// the files now hold exactly the draft. Steam has to be closed.
    pub fn save(&mut self) -> Result<SaveReport> {
        if steam::steam_is_running()? {
            bail!("Close Steam first: it overwrites its collection files while it runs.");
        }
        let mut cat = Catalog::read(&self.install)?;
        let current = cat.collections();
        if !same(&current, &self.base) {
            // Steam changed something since we loaded; keep both.
            self.draft = rebase(&self.base, &self.draft, &current);
            self.base = current.clone();
        }
        let ops = ops::plan_restore_exact(&current, &self.draft);
        if ops.is_empty() {
            self.persist();
            return Ok(SaveReport { snapshot: None, changes: 0, verified: true });
        }
        let folder = snapshot::save(&snapshot::store_dir()?, "before saving", current, &self.install.cloudstorage_dir())?;
        for op in &ops {
            cat.apply(op)?;
        }
        cat.write()?;

        let after = Catalog::read(&self.install)?.collections();
        let verified = same(&after, &self.draft);
        self.base = after.clone();
        self.draft = after;
        self.persist();
        if let Ok((config, text)) = config::load()
            && let Some(dir) = &config.backup_dir {
                crate::bundle::auto_backup(dir, self.base.clone(), Some(text), 10)?;
            }
        Ok(SaveReport { snapshot: folder.file_name().map(|n| n.to_string_lossy().into_owned()), changes: ops.len(), verified })
    }

    /// Steam's collections as they are on disk right now, ignoring the draft.
    pub fn saved_collections(&self) -> &[Collection] {
        &self.base
    }
}

/// What the online refresh needs from the session, taken so the network work can run without
/// holding the session.
#[derive(Debug, Clone)]
pub struct OnlineJob {
    pub steam64: u64,
    /// Games owned, with what decides whether their achievement page is worth fetching.
    pub games: Vec<online::AchievementHint>,
    pub country: String,
    pub settings: crate::settings::Settings,
    pub force: bool,
}

impl Session {
    pub fn online_job(&self, country: &str, force: bool) -> Result<OnlineJob> {
        Ok(OnlineJob {
            steam64: self.install.steam64(),
            games: self
                .games
                .iter()
                .filter(|g| g.is_game())
                .map(|g| online::AchievementHint {
                    appid: g.appid,
                    last_played: g.last_played,
                    playtime: g.playtime_minutes,
                    known_total: self.hunters.get(&g.appid).map_or(0, |h| h.achievements).max(g.achievements.map_or(0, |a| a.total)),
                    local_at: self.local_progress_at.get(&g.appid).copied().unwrap_or(0),
                    change: g.change,
                })
                .collect(),
            country: country.to_ascii_uppercase(),
            settings: crate::settings::load()?,
            force,
        })
    }
}

/// Runs every online source that is switched on and due. A source that fails (offline, private
/// profile) doesn't stop the others; its message is returned.
///
/// Sources on different sites run side by side, in three lanes: Steam's store, the community
/// profile (after SteamHunters, whose achievement counts decide which pages are worth asking
/// for), and ProtonDB. Each site still gets one request at a time with its usual pause, so this
/// is as polite as running them in a row, and a first run takes as long as the slowest lane
/// instead of all of them added up.
/// `saved` is told each time a source has finished and saved what it fetched, so the window
/// can show it straight away instead of waiting for the slowest source.
pub fn run_online(job: &OnlineJob, progress: &(dyn Fn(&str, usize, usize) + Sync), saved: &(dyn Fn() + Sync)) -> Vec<String> {
    let now = snapshot::now_secs();
    let errors = std::sync::Mutex::new(Vec::new());
    let fail = |e: String| errors.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(e);
    std::thread::scope(|scope| {
        scope.spawn(|| store_lane(job, now, progress, &fail, saved));
        scope.spawn(|| profile_lane(job, now, progress, &fail, saved));
        scope.spawn(|| proton_lane(job, now, progress, &fail, saved));
    });
    let mut errors = errors.into_inner().unwrap_or_else(std::sync::PoisonError::into_inner);
    // Stopping isn't a problem to report.
    errors.retain(|e| !e.ends_with("stopped"));
    if let Err(e) = online::record_usage(now) {
        errors.push(format!("Couldn't save the data usage: {e:#}"));
    }
    errors
}

type Progress<'a> = &'a (dyn Fn(&str, usize, usize) + Sync);
type Saved<'a> = &'a (dyn Fn() + Sync);

/// SteamHunters, then the achievement pages of the user's profile.
fn profile_lane(job: &OnlineJob, now: u64, progress: Progress, fail: &(dyn Fn(String) + Sync), saved: Saved) {
    let s = &job.settings;
    if s.online_hunters {
        let games: Vec<(u32, u32)> = job.games.iter().map(|g| (g.appid, g.change)).collect();
        match hunters::refresh(&games, now, job.force, &mut |d, t| progress("completion times", d, t)) {
            Ok(0) => {}
            Ok(_) => saved(),
            Err(e) => {
                fail(format!("Completion times: {e:#}"));
                saved();
            }
        }
    }
    if s.online_achievements {
        let cache: online::AchievementCache = online::load(online::ACHIEVEMENTS_FILE);
        // Counts SteamHunters just reported, so an update that added achievements is caught in
        // this run rather than the next.
        let hunted = hunters::load();
        // Only games that have been played and have achievements; anything else is 0 unlocked.
        let hints: Vec<online::AchievementHint> = job
            .games
            .iter()
            .map(|g| online::AchievementHint { known_total: g.known_total.max(hunted.stats.get(&g.appid).map_or(0, |h| h.achievements)), ..*g })
            .filter(|g| (g.last_played > 0 || g.playtime > 0) && g.known_total > 0)
            .collect();
        // A private profile is retried daily.
        let skip = cache.private && now.saturating_sub(cache.checked) < 24 * 3600 && !job.force;
        let mut due = if skip { Vec::new() } else { online::achievements_due(&cache, &hints, now, job.force) };
        // Most recently played first: on a long first run, the games the user cares about now
        // fill in first.
        due.sort_by_key(|h| std::cmp::Reverse(h.last_played));
        if !due.is_empty() {
            if let Err(e) = online::refresh_achievements(job.steam64, &due, now, &mut |d, t| progress("achievement progress", d, t)) {
                fail(format!("Achievements: {e:#}"));
            }
            saved();
        }
    }
}

fn proton_lane(job: &OnlineJob, now: u64, progress: Progress, fail: &(dyn Fn(String) + Sync), saved: Saved) {
    if job.settings.online_protondb {
        // One request per game, so only games the account owns.
        let ids: Vec<u32> = job.games.iter().map(|g| g.appid).collect();
        let due = online::proton_due(&online::load(online::PROTON_FILE), &ids, now, job.force);
        if !due.is_empty() {
            if let Err(e) = online::refresh_proton(&due, now, &mut |d, t| progress("ProtonDB ratings", d, t)) {
                fail(format!("ProtonDB: {e:#}"));
            }
            saved();
        }
    }
}

/// Store details (price, sale and its end date, review counts) for owned games, weekly.
fn store_lane(job: &OnlineJob, now: u64, progress: Progress, fail: &(dyn Fn(String) + Sync), saved: Saved) {
    if job.settings.online_prices {
        let ids: Vec<u32> = job.games.iter().map(|g| g.appid).collect();
        let mut cache: online::StoreCache = online::load(online::STORE_FILE);
        if !cache.country.eq_ignore_ascii_case(&job.country) {
            // Switch region straight away: prices kept from an earlier visit show at once,
            // and only what's missing or old is fetched below.
            cache.switch_to(&job.country);
            if let Err(e) = online::save(online::STORE_FILE, &cache) {
                fail(format!("Prices: {e:#}"));
            }
            saved();
        }
        let due = online::store_due(&cache, &ids, &job.country, now, online::WEEK, job.force);
        if !due.is_empty() {
            if let Err(e) = online::refresh_store(&due, &job.country, now, &mut |d, t| progress("store details", d, t)) {
                fail(format!("Store details: {e:#}"));
            }
            saved();
        }
    }
}

/// Same collections with the same names and games, regardless of order.
fn same(a: &[Collection], b: &[Collection]) -> bool {
    let key = |cols: &[Collection]| -> BTreeMap<String, (String, BTreeSet<u32>, bool)> {
        cols.iter().map(|c| (c.id.clone(), (c.name.clone(), c.apps.iter().copied().collect(), c.dynamic))).collect()
    };
    key(a) == key(b)
}

pub fn get_settings() -> Result<crate::settings::Settings> {
    crate::settings::load()
}

pub fn set_settings(settings: &crate::settings::Settings) -> Result<()> {
    crate::settings::save(settings)
}

#[derive(Debug, Serialize)]
pub struct SnapshotInfo {
    pub id: String,
    pub taken_at: u64,
    pub label: String,
    pub collections: usize,
    /// Games filed in the editable collections, each counted once, so entries can be told apart.
    pub games: usize,
}

pub fn history() -> Result<Vec<SnapshotInfo>> {
    Ok(snapshot::list(&snapshot::store_dir()?)?
        .into_iter()
        .map(|(path, s)| SnapshotInfo {
            id: path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            taken_at: s.taken_at,
            label: s.label,
            collections: s.collections.len(),
            games: s.collections.iter().filter(|c| !c.dynamic).flat_map(|c| c.apps.iter()).collect::<BTreeSet<_>>().len(),
        })
        .collect())
}

pub fn snapshot_collections(id: &str) -> Result<Vec<Collection>> {
    // The id comes from the UI, so refuse anything that could leave the snapshot folder.
    if id.contains(['/', '\\']) || id.contains("..") {
        bail!("invalid snapshot id");
    }
    let text = std::fs::read_to_string(snapshot::store_dir()?.join(id).join("collections.json")).context("snapshot not found")?;
    Ok(serde_json::from_str::<crate::model::Snapshot>(&text)?.collections)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn col(id: &str, name: &str, apps: &[u32]) -> Collection {
        Collection { id: id.into(), name: name.into(), apps: apps.to_vec(), dynamic: false }
    }

    #[test]
    fn rebase_keeps_both_sides() {
        let base = vec![col("uc-1", "A", &[1, 2]), col("uc-2", "B", &[3])];
        // The user added 9 to A and made C.
        let draft = vec![col("uc-1", "A", &[1, 2, 9]), col("uc-2", "B", &[3]), col("uc-3", "C", &[4])];
        // Meanwhile Steam removed 2 from A and renamed B.
        let current = vec![col("uc-1", "A", &[1]), col("uc-2", "Bee", &[3])];
        let out = rebase(&base, &draft, &current);
        let a = out.iter().find(|c| c.id == "uc-1").unwrap();
        assert_eq!(a.apps.iter().copied().collect::<BTreeSet<_>>(), [1, 9].into());
        assert_eq!(out.iter().find(|c| c.id == "uc-2").unwrap().name, "Bee");
        assert!(out.iter().any(|c| c.id == "uc-3"));
    }

    #[test]
    fn same_ignores_order() {
        assert!(same(&[col("uc-1", "A", &[1, 2])], &[col("uc-1", "A", &[2, 1])]));
        assert!(!same(&[col("uc-1", "A", &[1])], &[col("uc-1", "B", &[1])]));
    }
}
