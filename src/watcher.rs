//! Keeps an eye on Steam while the app is open, by looking at its files: playtime changes
//! when a game is played, and the collection file changes when Steam saves. After things
//! settle the rules are re-checked, and what happens next depends on the automation setting:
//! nothing, a notification, or putting the changes in and saving them once Steam is closed.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use anyhow::Result;
use serde::Serialize;

use crate::service::{self, OpView, Session};
use crate::settings::{self, Automation};
use crate::steam::{self, SteamInstall};

/// How often Steam is looked at. Each look only reads file times.
pub const TICK: Duration = Duration::from_secs(5);
/// How long things must stay quiet after a change before acting, so a game that is still
/// saving its playtime isn't evaluated half-way.
pub const SETTLE: Duration = Duration::from_secs(8);
/// Re-check at least this often, which catches time-based rules like "idle for a week".
pub const FULL_CHECK: Duration = Duration::from_secs(600);

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    /// Steam's files or running state changed; the library was re-read.
    Changed { steam_running: bool },
    /// The rules have changes to suggest, and the user chose to be asked.
    Pending { ops: Vec<OpView>, notes: Vec<String> },
    /// The rules' changes were put in the draft, and saved if Steam was closed.
    Organized { applied: usize, saved: bool, snapshot: Option<String> },
}
/// Decides when a re-check is due. Pure, so the timing rules are easy to test.
pub struct Tracker {
    running: u32,
    fingerprint: Option<u64>,
    changed_at: Option<Instant>,
    last_check: Option<Instant>,
}

impl Tracker {
    pub fn new() -> Self {
        Self { running: 0, fingerprint: None, changed_at: None, last_check: None }
    }

    /// Feed one look at Steam. Returns true when the rules should be re-checked now.
    pub fn observe(&mut self, now: Instant, running: u32, fingerprint: u64) -> bool {
        let first = self.fingerprint.is_none();
        if !first && (running != self.running || Some(fingerprint) != self.fingerprint) {
            // Every change restarts the quiet period.
            self.changed_at = Some(now);
        }
        self.running = running;
        self.fingerprint = Some(fingerprint);

        let settled = self.changed_at.is_some_and(|t| now.duration_since(t) >= SETTLE);
        let overdue = self.last_check.is_none_or(|t| now.duration_since(t) >= FULL_CHECK);
        if first || settled || overdue {
            self.changed_at = None;
            self.last_check = Some(now);
            true
        } else {
            false
        }
    }
}

impl Default for Tracker {
    fn default() -> Self {
        Self::new()
    }
}

fn signature<T: std::fmt::Debug>(ops: &T) -> u64 {
    let mut h = DefaultHasher::new();
    format!("{ops:?}").hash(&mut h);
    h.finish()
}

/// Changes whenever a game is played, Steam saves its collections, or Steam writes
/// achievement progress (after a play session, or when a game's library page is opened).
fn fingerprint(install: &SteamInstall) -> u64 {
    let mut h = DefaultHasher::new();
    for file in [install.config_dir().join("localconfig.vdf"), install.cloudstorage_dir().join("cloud-storage-namespace-1.json")] {
        std::fs::metadata(&file).and_then(|m| m.modified()).ok().hash(&mut h);
    }
    for dir in [install.config_dir().join("librarycache"), install.dir.join("appcache").join("stats")] {
        let newest = std::fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .filter_map(|e| e.metadata().and_then(|m| m.modified()).ok())
            .max();
        newest.hash(&mut h);
    }
    h.finish()
}

struct State {
    tracker: Tracker,
    steam_running: Option<bool>,
    /// The changes the user was last told about, so the same ones aren't announced twice.
    announced: Option<u64>,
}

fn tick(session: &Mutex<Option<Session>>, mode: Automation, state: &mut State, emit: &mut dyn FnMut(Event)) -> Result<()> {
    let install = SteamInstall::detect()?;
    let running = steam::steam_is_running().unwrap_or(false);
    let game = install.running_app_id().unwrap_or(0);
    let just_closed = state.steam_running == Some(true) && !running;
    let state_changed = state.steam_running != Some(running);
    state.steam_running = Some(running);
    let due = state.tracker.observe(Instant::now(), game, fingerprint(&install));
    if !due && !state_changed {
        return Ok(());
    }

    let mut guard = session.lock().map_err(|_| anyhow::anyhow!("session lock poisoned"))?;
    let Some(s) = guard.as_mut() else { return Ok(()) };
    if due {
        s.reload()?;
    }
    emit(Event::Changed { steam_running: running });

    match mode {
        Automation::Off => {}
        Automation::Notify if due => {
            let (ops, notes) = s.plan_rules()?;
            if ops.is_empty() {
                state.announced = None;
            } else if state.announced != Some(signature(&ops)) {
                state.announced = Some(signature(&ops));
                emit(Event::Pending { ops: service::describe(&ops, &s.games), notes });
            }
        }
        Automation::Notify => {}
        Automation::Auto => {
            let applied = if due { s.apply_rules()? } else { 0 };
            // Save when there is something to save and Steam is out of the way: right away,
            // or as soon as the user closes it.
            if !running && (applied > 0 || just_closed) && s.draft_state().pending > 0 {
                let report = s.save()?;
                emit(Event::Organized { applied: report.changes, saved: true, snapshot: report.snapshot });
            } else if applied > 0 {
                emit(Event::Organized { applied, saved: false, snapshot: None });
            }
        }
    }
    Ok(())
}

/// Runs until `stop` is set. The setting is re-read every tick, so changes take effect at once.
/// Problems such as a file being rewritten mid-read are expected and retried on the next tick.
pub fn run(session: &Mutex<Option<Session>>, stop: &AtomicBool, emit: &mut dyn FnMut(Event)) {
    let mut state = State { tracker: Tracker::new(), steam_running: None, announced: None };
    while !stop.load(Ordering::Relaxed) {
        let mode = settings::load().ok().and_then(|s| s.automation).unwrap_or(Automation::Off);
        let _ = tick(session, mode, &mut state, emit);
        let wake = Instant::now() + TICK;
        while Instant::now() < wake && !stop.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(200));
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_look_triggers_a_check_and_quiet_looks_do_not() {
        let mut t = Tracker::new();
        let t0 = Instant::now();
        assert!(t.observe(t0, 0, 1));
        assert!(!t.observe(t0 + Duration::from_secs(10), 0, 1));
        assert!(!t.observe(t0 + Duration::from_secs(20), 0, 1));
    }

    #[test]
    fn a_game_launch_is_checked_only_after_things_settle() {
        let mut t = Tracker::new();
        let t0 = Instant::now();
        t.observe(t0, 0, 1);
        assert!(!t.observe(t0 + Duration::from_secs(10), 570, 1), "launch seen, still settling");
        assert!(t.observe(t0 + Duration::from_secs(19), 570, 1), "quiet for the settle time");
        assert!(!t.observe(t0 + Duration::from_secs(29), 570, 1));
    }

    #[test]
    fn further_changes_restart_the_quiet_period() {
        let mut t = Tracker::new();
        let t0 = Instant::now();
        t.observe(t0, 0, 1);
        assert!(!t.observe(t0 + Duration::from_secs(10), 0, 2));
        assert!(!t.observe(t0 + Duration::from_secs(16), 0, 3), "changed again, window restarts");
        assert!(!t.observe(t0 + Duration::from_secs(20), 0, 3));
        assert!(t.observe(t0 + Duration::from_secs(25), 0, 3));
    }

    #[test]
    fn a_periodic_check_catches_time_based_rules() {
        let mut t = Tracker::new();
        let t0 = Instant::now();
        t.observe(t0, 0, 1);
        assert!(!t.observe(t0 + FULL_CHECK - Duration::from_secs(1), 0, 1));
        assert!(t.observe(t0 + FULL_CHECK, 0, 1));
    }

}
