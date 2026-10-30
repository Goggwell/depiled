//! Keeps an eye on Steam while the app is open. When a game is launched or closed, or its
//! playtime changes, the rules are re-checked after things settle. What happens next depends
//! on the user's automation setting: nothing, a notification, or applying the changes.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use anyhow::Result;
use serde::Serialize;

use crate::live::Live;
use crate::service::{self, OpView};
use crate::settings::{self, Automation};
use crate::steam::SteamInstall;

/// How often Steam is looked at. Each look is a cheap read of the games list.
pub const TICK: Duration = Duration::from_secs(10);
/// How long things must stay quiet after a change before the rules are re-checked, so a game
/// that is still saving its playtime isn't evaluated half-way.
pub const SETTLE: Duration = Duration::from_secs(8);
/// Re-check at least this often, which catches time-based rules like "idle for a week".
pub const FULL_CHECK: Duration = Duration::from_secs(600);

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    /// Changes are waiting, and the user chose to be asked.
    Pending { ops: Vec<OpView>, notes: Vec<String> },
    /// Changes were applied automatically.
    Organized { applied: usize, snapshot: Option<String>, verified: bool },
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

struct State {
    tracker: Tracker,
    /// The set of changes the user was last told about, so the same ones aren't announced twice.
    announced: Option<u64>,
}

fn tick(port: u16, mode: Automation, state: &mut State) -> Result<Option<Event>> {
    let live = Live::connect(port)?;
    let running = SteamInstall::detect().and_then(|i| i.running_app_id()).unwrap_or(0);
    if !state.tracker.observe(Instant::now(), running, live.fingerprint()?) {
        return Ok(None);
    }

    let plan = service::build_plan(port)?;
    if plan.ops.is_empty() {
        state.announced = None;
        return Ok(None);
    }
    match mode {
        Automation::Off => Ok(None),
        Automation::Notify => {
            let sig = signature(&plan.ops);
            if state.announced == Some(sig) {
                return Ok(None);
            }
            state.announced = Some(sig);
            Ok(Some(Event::Pending { ops: service::describe(&plan.ops, &plan.games), notes: plan.notes }))
        }
        Automation::Auto => {
            let report = service::apply_plan(plan)?;
            state.announced = None;
            Ok(Some(Event::Organized { applied: report.applied, snapshot: report.snapshot, verified: report.verified }))
        }
    }
}

/// Runs until `stop` is set. The setting is re-read every tick, so changes take effect at once.
/// Problems such as Steam not running are expected and simply retried on the next tick.
pub fn run(port: u16, stop: &AtomicBool, emit: &mut dyn FnMut(Event)) {
    let mut state = State { tracker: Tracker::new(), announced: None };
    while !stop.load(Ordering::Relaxed) {
        let mode = settings::load().ok().and_then(|s| s.automation).unwrap_or(Automation::Off);
        if mode == Automation::Off {
            state = State { tracker: Tracker::new(), announced: None };
        } else if let Ok(Some(event)) = tick(port, mode, &mut state) {
            emit(event);
        }

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
