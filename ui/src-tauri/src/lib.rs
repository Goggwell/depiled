//! Tauri shell: each command is a thin wrapper over `depiled::service`, run on a
//! blocking thread because the Steam connection is synchronous.

use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;

use serde::Serialize;
use depiled::cdp::DEFAULT_PORT;
use depiled::model::Achievements;
use depiled::service::{self, Change, Library, OpView, RestoreReport, SmartResult, SnapshotInfo, Status};
use depiled::settings::Settings;
use depiled::smart::SmartRule;
use depiled::steam::SteamInstall;
use depiled::watcher;
use tauri::{Emitter, Manager};

static STOP_WATCHER: AtomicBool = AtomicBool::new(false);

type Res<T> = Result<T, String>;

async fn blocking<T, F>(f: F) -> Res<T>
where
    T: Send + 'static,
    F: FnOnce() -> anyhow::Result<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
async fn status() -> Res<Status> {
    blocking(|| Ok(service::status(DEFAULT_PORT))).await
}

#[tauri::command]
async fn enable_setup() -> Res<()> {
    blocking(service::enable_debug_port).await
}

#[tauri::command]
async fn load_library() -> Res<Library> {
    blocking(|| service::load_library(DEFAULT_PORT)).await
}

#[tauri::command]
async fn load_achievements() -> Res<BTreeMap<u32, Achievements>> {
    blocking(|| service::achievements(DEFAULT_PORT)).await
}

/// Snapshot id of the checkpoint taken before the change, or none if nothing changed.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
async fn move_games(appids: Vec<u32>, add_to: Vec<String>, remove_from: Vec<String>, label: String) -> Res<Option<String>> {
    blocking(move || service::change_membership(DEFAULT_PORT, Change { appids, add_to, remove_from }, &label)).await
}

#[tauri::command]
async fn create_collection(name: String, appids: Vec<u32>) -> Res<Option<String>> {
    blocking(move || service::create_collection(DEFAULT_PORT, &name, appids)).await
}

#[tauri::command]
async fn rename_collection(id: String, name: String) -> Res<Option<String>> {
    blocking(move || service::rename_collection(DEFAULT_PORT, &id, &name)).await
}

#[tauri::command]
async fn delete_collection(id: String) -> Res<Option<String>> {
    blocking(move || service::delete_collection(DEFAULT_PORT, &id)).await
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
async fn smart_rules() -> Res<BTreeMap<String, SmartRule>> {
    blocking(|| service::smart_rules(DEFAULT_PORT)).await
}

#[tauri::command]
async fn create_smart_collection(name: String, rule: SmartRule, auto: bool) -> Res<SmartResult> {
    blocking(move || service::create_smart_collection(DEFAULT_PORT, &name, &rule, auto)).await
}

#[tauri::command]
async fn set_smart_rule(id: String, rule: Option<SmartRule>) -> Res<()> {
    blocking(move || service::set_smart_rule(DEFAULT_PORT, &id, rule)).await
}

#[tauri::command]
async fn fill_collection(id: String) -> Res<SmartResult> {
    blocking(move || service::fill_collection(DEFAULT_PORT, &id)).await
}

/// (games matching in total, games that would be newly added)
#[tauri::command]
async fn preview_smart(rule: SmartRule, collection: Option<String>) -> Res<(usize, usize)> {
    blocking(move || service::preview_smart(DEFAULT_PORT, &rule, collection.as_deref())).await
}

#[tauri::command]
async fn history() -> Res<Vec<SnapshotInfo>> {
    blocking(service::history).await
}

#[tauri::command]
async fn restore_snapshot(id: String) -> Res<RestoreReport> {
    blocking(move || {
        let target = service::snapshot_collections(&id)?;
        service::restore(DEFAULT_PORT, &target, true, true, "undo")
    })
    .await
}

#[derive(Serialize)]
struct RulesPreview {
    ops: Vec<OpView>,
    notes: Vec<String>,
}

#[tauri::command]
async fn preview_rules() -> Res<RulesPreview> {
    blocking(|| service::preview_rules(DEFAULT_PORT).map(|(ops, notes)| RulesPreview { ops, notes })).await
}

#[tauri::command]
async fn apply_rules() -> Res<service::ApplyReport> {
    blocking(|| service::apply_rules(DEFAULT_PORT)).await
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // Let the page show Steam's cached cover art, and nothing else on disk.
            if let Ok(install) = SteamInstall::detect() {
                let covers = install.dir.join("appcache").join("librarycache");
                app.asset_protocol_scope().allow_directory(covers, true)?;
            }

            // Watch Steam in the background. The window hears about it through an event, and
            // what happens is decided by the user's automation setting.
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                watcher::run(DEFAULT_PORT, &STOP_WATCHER, &mut |event| {
                    let _ = handle.emit("organizer", &event);
                });
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            set_settings,
            status,
            enable_setup,
            load_library,
            load_achievements,
            move_games,
            create_collection,
            rename_collection,
            delete_collection,
            smart_rules,
            create_smart_collection,
            set_smart_rule,
            fill_collection,
            preview_smart,
            history,
            restore_snapshot,
            preview_rules,
            apply_rules,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the app");
}
