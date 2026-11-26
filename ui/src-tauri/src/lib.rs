//! Tauri shell: each command is a thin wrapper over `depiled::service`, run on a blocking
//! thread because reading Steam's files is synchronous. One `Session` holds the library and
//! the unsaved draft for the whole app.

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use depiled::service::{self, CollectionView, DraftState, Library, OpView, SaveReport, Session, SnapshotInfo, Status};
use depiled::settings::Settings;
use depiled::smart::SmartRule;
use depiled::steam::SteamInstall;
use depiled::watcher;
use serde::Serialize;
use tauri::{Emitter, Manager, State};

static STOP_WATCHER: AtomicBool = AtomicBool::new(false);
/// An online refresh is running, so a second one isn't started on top.
static ONLINE: AtomicBool = AtomicBool::new(false);

type Res<T> = Result<T, String>;
type Shared = Arc<Mutex<Option<Session>>>;

async fn blocking<T, F>(f: F) -> Res<T>
where
    T: Send + 'static,
    F: FnOnce() -> anyhow::Result<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())?.map_err(|e| format!("{e:#}"))
}

/// Runs `f` with the session, opening it first if needed.
async fn with<T, F>(state: State<'_, Shared>, f: F) -> Res<T>
where
    T: Send + 'static,
    F: FnOnce(&mut Session) -> anyhow::Result<T> + Send + 'static,
{
    let shared = state.inner().clone();
    blocking(move || {
        let mut guard = shared.lock().map_err(|_| anyhow::anyhow!("the app's state is unavailable, restart it"))?;
        if guard.is_none() {
            *guard = Some(Session::open()?);
        }
        f(guard.as_mut().expect("just opened"))
    })
    .await
}

/// What an edit changed, so the window can update without reloading the whole library.
#[derive(Serialize)]
struct Patch {
    collections: Vec<CollectionView>,
    draft: DraftState,
    /// New collection lists for the games whose membership changed.
    membership: HashMap<u32, Vec<String>>,
    /// Result of the edit, e.g. a new collection's id.
    value: Option<String>,
}

fn edit(s: &mut Session, f: impl FnOnce(&mut Session) -> anyhow::Result<Option<String>>) -> anyhow::Result<Patch> {
    let before = service::membership(s.collections());
    let value = f(s)?;
    let mut after = service::membership(s.collections());
    let mut changed: HashMap<u32, Vec<String>> = HashMap::new();
    for app in before.keys().chain(after.keys()).copied().collect::<std::collections::BTreeSet<_>>() {
        if before.get(&app) != after.get(&app) {
            changed.insert(app, after.remove(&app).unwrap_or_default());
        }
    }
    Ok(Patch { collections: s.collection_views(), draft: s.draft_state(), membership: changed, value })
}

#[tauri::command]
async fn status() -> Res<Status> {
    blocking(|| Ok(service::status())).await
}

#[tauri::command]
async fn disable_debug_port() -> Res<()> {
    blocking(service::disable_debug_port).await
}

/// With `online_only`, after an online refresh, only the cached online data is read again:
/// Steam's own files haven't changed.
#[tauri::command]
async fn load_library(state: State<'_, Shared>, online_only: Option<bool>) -> Res<Library> {
    with(state, move |s| {
        if online_only == Some(true) {
            s.reload_online();
        } else {
            s.reload()?;
        }
        Ok(s.library())
    })
    .await
}

#[tauri::command]
async fn move_games(state: State<'_, Shared>, appids: Vec<u32>, add_to: Vec<String>, remove_from: Vec<String>, label: String) -> Res<Patch> {
    with(state, move |s| edit(s, |s| s.change_membership(&appids, &add_to, &remove_from, &label).map(|()| None))).await
}

#[tauri::command]
async fn create_collection(state: State<'_, Shared>, name: String, appids: Vec<u32>) -> Res<Patch> {
    with(state, move |s| edit(s, |s| s.create_collection(&name, appids).map(Some))).await
}

#[tauri::command]
async fn rename_collection(state: State<'_, Shared>, id: String, name: String) -> Res<Patch> {
    with(state, move |s| edit(s, |s| s.rename_collection(&id, &name).map(|()| None))).await
}

#[tauri::command]
async fn delete_collection(state: State<'_, Shared>, id: String) -> Res<Patch> {
    with(state, move |s| edit(s, |s| s.delete_collection(&id).map(|()| None))).await
}

#[tauri::command]
async fn duplicate_collection(state: State<'_, Shared>, id: String) -> Res<Patch> {
    with(state, move |s| edit(s, |s| s.duplicate_collection(&id).map(Some))).await
}

#[tauri::command]
async fn merge_collections(state: State<'_, Shared>, sources: Vec<String>, into: String) -> Res<Patch> {
    with(state, move |s| edit(s, |s| s.merge_collections(&sources, &into).map(|()| None))).await
}

#[tauri::command]
async fn set_color(state: State<'_, Shared>, id: String, color: Option<String>) -> Res<Patch> {
    with(state, move |s| edit(s, |s| s.set_color(&id, color).map(|()| None))).await
}

#[tauri::command]
async fn undo(state: State<'_, Shared>) -> Res<Patch> {
    with(state, |s| edit(s, |s| Ok(s.undo()))).await
}

#[tauri::command]
async fn redo(state: State<'_, Shared>) -> Res<Patch> {
    with(state, |s| edit(s, |s| Ok(s.redo()))).await
}

#[tauri::command]
async fn discard(state: State<'_, Shared>) -> Res<Patch> {
    with(state, |s| {
        edit(s, |s| {
            s.discard();
            Ok(None)
        })
    })
    .await
}

#[tauri::command]
async fn pending(state: State<'_, Shared>) -> Res<Vec<OpView>> {
    with(state, |s| Ok(s.pending())).await
}

/// Saves the draft. With `close_steam`, asks Steam to exit first and, if the user wants,
/// starts it again afterwards.
#[tauri::command]
async fn save(state: State<'_, Shared>, close_steam: bool) -> Res<SaveReport> {
    with(state, move |s| {
        let was_running = depiled::steam::steam_is_running().unwrap_or(false);
        if close_steam {
            service::close_steam(Duration::from_secs(60))?;
        }
        let report = s.save()?;
        if close_steam && was_running && service::get_settings().map(|x| x.reopen_steam).unwrap_or(true) {
            service::start_steam()?;
        }
        Ok(report)
    })
    .await
}

#[tauri::command]
async fn get_settings() -> Res<Settings> {
    blocking(service::get_settings).await
}

#[tauri::command]
async fn set_settings(settings: Settings) -> Res<()> {
    blocking(move || service::set_settings(&settings)).await
}

#[tauri::command]
async fn smart_rules(state: State<'_, Shared>) -> Res<BTreeMap<String, SmartRule>> {
    with(state, |s| s.smart_rules()).await
}

#[tauri::command]
async fn create_smart_collection(state: State<'_, Shared>, name: String, rule: SmartRule, auto: bool) -> Res<Patch> {
    with(state, move |s| {
        edit(s, |s| {
            let (id, added) = s.create_smart_collection(&name, &rule, auto)?;
            Ok(Some(format!("{id}:{added}")))
        })
    })
    .await
}

#[tauri::command]
async fn set_smart_rule(state: State<'_, Shared>, id: String, rule: Option<SmartRule>) -> Res<()> {
    with(state, move |s| s.set_smart_rule(&id, rule)).await
}

#[tauri::command]
async fn fill_collection(state: State<'_, Shared>, id: String) -> Res<Patch> {
    with(state, move |s| edit(s, |s| s.fill_collection(&id).map(|n| Some(n.to_string())))).await
}

/// (games matching in total, games that would be newly added)
#[tauri::command]
async fn preview_smart(state: State<'_, Shared>, rule: SmartRule, collection: Option<String>) -> Res<(usize, usize)> {
    with(state, move |s| s.preview_smart(&rule, collection.as_deref())).await
}

#[tauri::command]
async fn history() -> Res<Vec<SnapshotInfo>> {
    blocking(service::history).await
}

#[tauri::command]
async fn restore_snapshot(state: State<'_, Shared>, id: String) -> Res<Patch> {
    with(state, move |s| edit(s, |s| s.restore_snapshot(&id).map(|()| None))).await
}

#[derive(Serialize)]
struct RulesPreview {
    ops: Vec<OpView>,
    notes: Vec<String>,
}

#[tauri::command]
async fn preview_rules(state: State<'_, Shared>) -> Res<RulesPreview> {
    with(state, |s| s.preview_rules().map(|(ops, notes)| RulesPreview { ops, notes })).await
}

#[tauri::command]
async fn apply_rules(state: State<'_, Shared>) -> Res<Patch> {
    with(state, |s| edit(s, |s| s.apply_rules().map(|n| Some(n.to_string())))).await
}

#[derive(Clone, Serialize)]
struct OnlineProgress {
    /// What is being fetched, in words ("prices and reviews").
    task: String,
    /// Each source still fetching, by its task name.
    active: Vec<String>,
    done: usize,
    total: usize,
    finished: bool,
    errors: Vec<String>,
    changed: bool,
}

/// Fetches whichever online extras are switched on and due, in the background. Cheap to call
/// often: sources that are fresh are skipped without a request. Progress arrives as "online"
/// events; the window reloads the library when it finishes. Returns false if a run is already
/// going.
#[tauri::command]
async fn refresh_online(app: tauri::AppHandle, state: State<'_, Shared>, country: String, force: bool) -> Res<bool> {
    let job = with(state, move |s| s.online_job(&country, force)).await?;
    if job.settings.online_stats != Some(true) || ONLINE.swap(true, Ordering::SeqCst) {
        return Ok(false);
    }
    depiled::online::CANCEL.store(false, Ordering::Relaxed);
    std::thread::spawn(move || {
        // Sources run side by side, so progress is added up across them: the window shows the
        // ones still going and how far they are together.
        let lanes: Mutex<BTreeMap<String, (usize, usize)>> = Mutex::new(BTreeMap::new());
        let changed = AtomicBool::new(false);
        let errors = service::run_online(&job, &|task, done, total| {
            changed.store(true, Ordering::Relaxed);
            let mut lanes = lanes.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            lanes.insert(task.to_owned(), (done, total));
            let active: Vec<String> = lanes.iter().filter(|(_, (d, t))| d < t || *t == 0).map(|(k, _)| k.clone()).collect();
            let task = match active.as_slice() {
                [] => lanes.keys().last().cloned().unwrap_or_default(),
                [one] => one.clone(),
                [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
            };
            let (done, total) = lanes.values().fold((0, 0), |(d, t), (dd, tt)| (d + dd, t + tt));
            let _ = app.emit("online", OnlineProgress { task, active, done, total, finished: false, errors: Vec::new(), changed: true });
        });
        ONLINE.store(false, Ordering::SeqCst);
        let changed = changed.load(Ordering::Relaxed) || !errors.is_empty();
        let _ = app.emit("online", OnlineProgress { task: String::new(), active: Vec::new(), done: 0, total: 0, finished: true, changed, errors });
    });
    Ok(true)
}
/// Stops the online refresh after the request in flight. What was fetched so far is kept.
#[tauri::command]
fn stop_online() {
    depiled::online::CANCEL.store(true, Ordering::Relaxed);
}

pub fn run() {
    let shared: Shared = Arc::new(Mutex::new(None));
    let for_watcher = shared.clone();
    tauri::Builder::default()
        .manage(shared)
        .setup(move |app| {
            // Let the page show Steam's cached cover art, and nothing else on disk.
            if let Ok(install) = SteamInstall::detect() {
                let covers = install.dir.join("appcache").join("librarycache");
                app.asset_protocol_scope().allow_directory(covers, true)?;
            }

            // Watch Steam's files in the background. The window hears about it through an
            // event, and what happens is decided by the user's automation setting.
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                watcher::run(&for_watcher, &STOP_WATCHER, &mut |event| {
                    let _ = handle.emit("organizer", &event);
                });
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            status,
            disable_debug_port,
            get_settings,
            set_settings,
            load_library,
            move_games,
            create_collection,
            rename_collection,
            delete_collection,
            duplicate_collection,
            merge_collections,
            set_color,
            undo,
            redo,
            discard,
            pending,
            save,
            smart_rules,
            create_smart_collection,
            set_smart_rule,
            fill_collection,
            preview_smart,
            history,
            restore_snapshot,
            preview_rules,
            apply_rules,
            refresh_online,
            stop_online,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the app");
}
