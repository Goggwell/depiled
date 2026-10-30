//! High-level operations shared by the CLI and the desktop UI. Every change takes a snapshot
//! first, so each one can be undone.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::live::Live;
use crate::model::{Achievements, Collection, Game};
use crate::ops::{self, Op};
use crate::steam::{self, SteamInstall};
use crate::{cdp, config, rules, smart, snapshot};

const FLAG_FILE: &str = ".cef-enable-remote-debugging";

#[derive(Debug, Serialize)]
pub struct Status {
    pub steam_found: bool,
    pub steam_running: bool,
    /// The debug port answers, so collections can be read and edited.
    pub ready: bool,
    pub flag_file: bool,
}

pub fn status(port: u16) -> Status {
    let install = SteamInstall::detect().ok();
    Status {
        steam_found: install.is_some(),
        steam_running: steam::steam_is_running().unwrap_or(false),
        ready: cdp::shared_context_url(port).is_ok(),
        flag_file: install.is_some_and(|i| i.dir.join(FLAG_FILE).exists()),
    }
}

/// One-time setup: lets this app talk to the running Steam client. Steam must be restarted
/// afterwards. Delete the file to turn it off again.
pub fn enable_debug_port() -> Result<()> {
    let install = SteamInstall::detect()?;
    std::fs::write(install.dir.join(FLAG_FILE), b"").context("could not create the Steam flag file")
}

pub fn disable_debug_port() -> Result<()> {
    let path = SteamInstall::detect()?.dir.join(FLAG_FILE);
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct GameView {
    pub appid: u32,
    pub name: String,
    pub playtime_minutes: u32,
    pub last_played: u64,
    pub categories: Vec<u32>,
    pub tags: Vec<u32>,
    pub metacritic: u32,
    pub review_percent: u32,
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
}

/// Which editable collections each game is in. One pass over the collections, instead of
/// searching every collection for every game, which is the difference between milliseconds and
/// many seconds once a library has thousands of games.
pub fn membership(collections: &[Collection]) -> std::collections::HashMap<u32, Vec<String>> {
    let mut map: std::collections::HashMap<u32, Vec<String>> = std::collections::HashMap::new();
    for c in collections.iter().filter(|c| !c.dynamic) {
        for app in &c.apps {
            map.entry(*app).or_default().push(c.id.clone());
        }
    }
    map
}

pub fn load_library(port: u16) -> Result<Library> {
    let live = Live::connect(port)?;
    let collections = live.collections()?;
    let games = live.games()?;
    let tags = live.tag_names()?;
    let steam_dir = SteamInstall::detect().ok().map(|i| i.dir);
    let mut member_of = membership(&collections);

    let game_views = games
        .into_iter()
        .map(|g| GameView {
            cover: steam_dir.as_deref().and_then(|d| cover_path(d, g.appid)),
            collections: member_of.remove(&g.appid).unwrap_or_default(),
            appid: g.appid,
            name: g.name,
            playtime_minutes: g.playtime_minutes,
            last_played: g.last_played,
            categories: g.categories,
            tags: g.tags,
            metacritic: g.metacritic,
            review_percent: g.review_percent,
        })
        .collect();

    Ok(Library {
        games: game_views,
        collections: collections
            .iter()
            .map(|c| CollectionView { id: c.id.clone(), name: c.name.clone(), count: c.apps.len(), dynamic: c.dynamic })
            .collect(),
        tags,
        flags: rules::FLAGS.iter().map(|(n, id)| FlagView { id: *id, name: (*n).to_owned() }).collect(),
    })
}

/// Achievement progress for games that have been started. Can take a few seconds when
/// Steam's cache is cold, so the UI asks for it separately.
pub fn achievements(port: u16) -> Result<BTreeMap<u32, Achievements>> {
    let live = Live::connect(port)?;
    let started: Vec<u32> = live.games()?.iter().filter(|g| g.started()).map(|g| g.appid).collect();
    live.achievements(&started)
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

/// Takes a snapshot of the live state, applies `ops`, and returns the snapshot's id. Returns
/// `None` when there was nothing to do (no snapshot is taken then).
fn run_ops(live: &Live, current: Vec<Collection>, ops: Vec<Op>, label: &str) -> Result<Option<String>> {
    if ops.is_empty() {
        return Ok(None);
    }
    let install = SteamInstall::detect()?;
    let folder = snapshot::save(&snapshot::store_dir()?, label, current, &install.cloudstorage_dir())?;
    for op in &ops {
        live.apply(op)?;
    }
    Ok(folder.file_name().map(|n| n.to_string_lossy().into_owned()))
}

fn find<'a>(current: &'a [Collection], id: &str) -> Result<&'a Collection> {
    let c = current.iter().find(|c| c.id == id).with_context(|| format!("collection {id} not found"))?;
    if c.dynamic {
        bail!("'{}' is a dynamic collection managed by Steam, so it can't be edited here", c.name);
    }
    Ok(c)
}

fn name_taken(current: &[Collection], name: &str, except: Option<&str>) -> bool {
    current.iter().any(|c| c.name.eq_ignore_ascii_case(name.trim()) && Some(c.id.as_str()) != except)
}

pub struct Change {
    pub appids: Vec<u32>,
    pub add_to: Vec<String>,
    pub remove_from: Vec<String>,
}

pub fn change_membership(port: u16, change: Change, label: &str) -> Result<Option<String>> {
    let live = Live::connect(port)?;
    let current = live.collections()?;
    let mut plan = Vec::new();
    for id in &change.remove_from {
        let c = find(&current, id)?;
        let apps: Vec<u32> = change.appids.iter().copied().filter(|a| c.apps.contains(a)).collect();
        if !apps.is_empty() {
            plan.push(Op::Remove { id: c.id.clone(), name: c.name.clone(), apps });
        }
    }
    for id in &change.add_to {
        let c = find(&current, id)?;
        let apps: Vec<u32> = change.appids.iter().copied().filter(|a| !c.apps.contains(a)).collect();
        if !apps.is_empty() {
            plan.push(Op::Add { id: c.id.clone(), name: c.name.clone(), apps });
        }
    }
    run_ops(&live, current, plan, label)
}

pub fn create_collection(port: u16, name: &str, appids: Vec<u32>) -> Result<Option<String>> {
    let name = name.trim();
    if name.is_empty() {
        bail!("give the collection a name");
    }
    let live = Live::connect(port)?;
    let current = live.collections()?;
    if name_taken(&current, name, None) {
        bail!("a collection called '{name}' already exists");
    }
    run_ops(&live, current, vec![Op::Create { name: name.to_owned(), apps: appids }], &format!("create {name}"))
}

pub fn rename_collection(port: u16, id: &str, name: &str) -> Result<Option<String>> {
    let name = name.trim();
    if name.is_empty() {
        bail!("give the collection a name");
    }
    let live = Live::connect(port)?;
    let current = live.collections()?;
    let from = find(&current, id)?.name.clone();
    if name_taken(&current, name, Some(id)) {
        bail!("a collection called '{name}' already exists");
    }
    let op = Op::Rename { id: id.to_owned(), from: from.clone(), to: name.to_owned() };
    let snapshot = run_ops(&live, current, vec![op], &format!("rename {name}"))?;
    // Keep any auto-fill criteria attached to the renamed collection.
    let mut all = smart::load()?;
    if let Some(rule) = all.remove(&smart::key(&from)) {
        all.insert(smart::key(name), rule);
        smart::save(&all)?;
    }
    Ok(snapshot)
}

pub fn delete_collection(port: u16, id: &str) -> Result<Option<String>> {
    let live = Live::connect(port)?;
    let current = live.collections()?;
    let name = find(&current, id)?.name.clone();
    run_ops(&live, current, vec![Op::Delete { id: id.to_owned(), name: name.clone() }], &format!("delete {name}"))
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
}

pub fn history() -> Result<Vec<SnapshotInfo>> {
    Ok(snapshot::list(&snapshot::store_dir()?)?
        .into_iter()
        .map(|(path, s)| SnapshotInfo {
            id: path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            taken_at: s.taken_at,
            label: s.label,
            collections: s.collections.len(),
        })
        .collect())
}

pub fn snapshot_collections(id: &str) -> Result<Vec<Collection>> {
    // The id comes from the UI, so refuse anything that could leave the snapshot folder.
    if id.contains(['/', '\\']) || id.contains("..") {
        bail!("invalid snapshot id");
    }
    let text = std::fs::read_to_string(snapshot::store_dir()?.join(id).join("collections.json"))
        .context("snapshot not found")?;
    Ok(serde_json::from_str::<crate::model::Snapshot>(&text)?.collections)
}

#[derive(Debug, Serialize)]
pub struct RestoreReport {
    pub changes: Vec<String>,
    pub safety_snapshot: Option<String>,
    pub verified: bool,
}

/// Brings the live collections back to `target`. With `apply` false, only reports the changes.
/// With `exact`, collections created since the snapshot are deleted too.
pub fn restore(port: u16, target: &[Collection], apply: bool, exact: bool, safety_label: &str) -> Result<RestoreReport> {
    let live = Live::connect(port)?;
    let current = live.collections()?;
    let diff = if exact { ops::plan_restore_exact } else { ops::plan_restore };
    let plan = diff(&current, target);
    let changes: Vec<String> = plan.iter().map(ToString::to_string).collect();
    if plan.is_empty() || !apply {
        return Ok(RestoreReport { changes, safety_snapshot: None, verified: plan.is_empty() });
    }
    let safety = run_ops(&live, current, plan, safety_label)?;
    let verified = diff(&live.collections()?, target).is_empty();
    Ok(RestoreReport { changes, safety_snapshot: safety, verified })
}

pub struct PlanData {
    pub live: Live,
    pub current: Vec<Collection>,
    pub games: Vec<Game>,
    pub ops: Vec<Op>,
    pub config_text: String,
    pub config: config::Config,
    /// The rules that produced `ops`, with tag names resolved to ids.
    pub rules: Vec<rules::Rule>,
    pub notes: Vec<String>,
}

/// Everything the user's rules ask for: the config file's rules plus each smart collection's.
pub fn build_plan(port: u16) -> Result<PlanData> {
    let (config, config_text) = config::load()?;
    let mut all = config.rules.clone();
    // Criteria for a collection that no longer exists are ignored, so deleting a collection
    // isn't undone by the next Auto-organize. They apply again if it comes back (e.g. via undo).
    let existing: std::collections::BTreeSet<String> =
        Live::connect(port)?.collections()?.iter().filter(|c| !c.dynamic).map(|c| smart::key(&c.name)).collect();
    for (name, smart) in smart::load()?.into_iter().filter(|(name, _)| existing.contains(name)) {
        all.push(smart.to_rule(&name)?);
    }
    gather(port, all, config, config_text)
}

fn gather(port: u16, mut rules: Vec<rules::Rule>, config: config::Config, config_text: String) -> Result<PlanData> {
    let live = Live::connect(port)?;
    rules::resolve_tags(&mut rules, &live.tag_names()?)?;
    let current = live.collections()?;
    let mut games = live.games()?;
    let mut notes = Vec::new();

    if rules.iter().any(|r| r.all_achievements.is_some()) {
        let started: Vec<u32> = games.iter().filter(|g| g.started()).map(|g| g.appid).collect();
        let progress = live.achievements(&started)?;
        for g in &mut games {
            g.achievements = progress.get(&g.appid).copied();
        }
        let missing = started.len() - progress.len();
        if missing > 0 {
            notes.push(format!("Steam hasn't reported achievements for {missing} games yet; they won't count as completed."));
        }
    }

    let ops = rules::plan(&rules, &games, &current, snapshot::now_secs());
    Ok(PlanData { live, current, games, ops, config_text, config, rules, notes })
}

#[derive(Debug, Serialize)]
pub struct SmartResult {
    pub snapshot: Option<String>,
    /// Games added by this step.
    pub added: usize,
}

fn added_count(ops: &[Op]) -> usize {
    ops.iter()
        .map(|op| match op {
            Op::Create { apps, .. } | Op::Add { apps, .. } => apps.len(),
            _ => 0,
        })
        .sum()
}

/// Creates a collection filled with every game matching `smart`. With `auto`, the criteria
/// are remembered so the collection can be topped up later.
pub fn create_smart_collection(port: u16, name: &str, smart: &smart::SmartRule, auto: bool) -> Result<SmartResult> {
    let name = name.trim();
    if name.is_empty() {
        bail!("give the collection a name");
    }
    if smart.is_empty() {
        bail!("pick at least one filter first");
    }
    let rule = smart.to_rule(name)?;
    let probe = Live::connect(port)?.collections()?;
    if name_taken(&probe, name, None) {
        bail!("a collection called '{name}' already exists");
    }
    let plan = gather(port, vec![rule], config::Config::default(), String::new())?;
    let added = added_count(&plan.ops);
    // A collection with no matches yet is still created, so it exists to be filled later.
    let ops = if plan.ops.is_empty() { vec![Op::Create { name: name.to_owned(), apps: Vec::new() }] } else { plan.ops };
    let snapshot = run_ops(&plan.live, plan.current, ops, &format!("create {name}"))?;
    if auto {
        let mut all = smart::load()?;
        all.insert(smart::key(name), smart.clone());
        smart::save(&all)?;
    }
    Ok(SmartResult { snapshot, added })
}

/// Smart rules by collection id, for the collections that currently exist.
pub fn smart_rules(port: u16) -> Result<BTreeMap<String, smart::SmartRule>> {
    let stored = smart::load()?;
    let current = Live::connect(port)?.collections()?;
    Ok(current
        .into_iter()
        .filter(|c| !c.dynamic)
        .filter_map(|c| stored.get(&smart::key(&c.name)).map(|r| (c.id, r.clone())))
        .collect())
}

/// Sets or clears (with `None`) what a collection automatically collects.
pub fn set_smart_rule(port: u16, id: &str, rule: Option<smart::SmartRule>) -> Result<()> {
    let current = Live::connect(port)?.collections()?;
    let name = find(&current, id)?.name.clone();
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

/// Adds every game that fits the collection's saved criteria and isn't in it yet.
pub fn fill_collection(port: u16, id: &str) -> Result<SmartResult> {
    let current = Live::connect(port)?.collections()?;
    let name = find(&current, id)?.name.clone();
    let smart = smart::load()?.remove(&smart::key(&name)).with_context(|| format!("'{name}' has no auto-fill criteria yet"))?;
    let plan = gather(port, vec![smart.to_rule(&name)?], config::Config::default(), String::new())?;
    let added = added_count(&plan.ops);
    let snapshot = run_ops(&plan.live, plan.current, plan.ops, &format!("fill {name}"))?;
    Ok(SmartResult { snapshot, added })
}

/// How many games the criteria would put in the collection right now, and how many of those
/// aren't in `collection` yet. Read-only.
pub fn preview_smart(port: u16, smart: &smart::SmartRule, collection: Option<&str>) -> Result<(usize, usize)> {
    let name = collection.unwrap_or("(preview)");
    let plan = gather(port, vec![smart.to_rule(name)?], config::Config::default(), String::new())?;
    let new = added_count(&plan.ops);
    let total = plan.games.iter().filter(|g| plan.rules[0].matches(g, snapshot::now_secs())).count();
    Ok((total, new))
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
    let names: BTreeMap<u32, &str> = games.iter().map(|g| (g.appid, g.name.as_str())).collect();
    ops.iter()
        .map(|op| {
            let (kind, collection, apps): (&'static str, &str, &[u32]) = match op {
                Op::Create { name, apps } => ("create", name, apps),
                Op::Add { name, apps, .. } => ("add", name, apps),
                Op::Remove { name, apps, .. } => ("remove", name, apps),
                Op::Rename { to, .. } => ("rename", to, &[]),
                Op::Delete { name, .. } => ("delete", name, &[]),
            };
            OpView {
                kind,
                collection: collection.to_owned(),
                count: apps.len(),
                sample: apps.iter().take(6).filter_map(|a| names.get(a)).map(|n| (*n).to_owned()).collect(),
            }
        })
        .collect()
}

pub fn preview_rules(port: u16) -> Result<(Vec<OpView>, Vec<String>)> {
    let plan = build_plan(port)?;
    Ok((describe(&plan.ops, &plan.games), plan.notes))
}

#[derive(Debug, Serialize)]
pub struct ApplyReport {
    pub snapshot: Option<String>,
    pub applied: usize,
    pub verified: bool,
}

pub fn apply_rules(port: u16) -> Result<ApplyReport> {
    apply_plan(build_plan(port)?)
}

/// Applies an already-built plan: snapshot first, then the changes, then a check that the
/// result follows the rules.
pub fn apply_plan(plan: PlanData) -> Result<ApplyReport> {
    let applied = plan.ops.len();
    let snapshot = run_ops(&plan.live, plan.current, plan.ops, "auto-organize")?;
    let after = plan.live.collections()?;
    let verified = rules::plan(&plan.rules, &plan.games, &after, snapshot::now_secs()).is_empty();
    if let Some(dir) = &plan.config.backup_dir {
        crate::bundle::auto_backup(dir, after, Some(plan.config_text), 10)?;
    }
    Ok(ApplyReport { snapshot, applied, verified })
}
