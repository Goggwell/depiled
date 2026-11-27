//! Optional online extras, none of which need an account or API key. Each is fetched now and
//! then, politely and in batches where the source allows it, and kept on disk so the app works
//! the same offline with whatever was fetched last.
//!
//! - Achievement progress: the public achievements page of the user's own Steam profile.
//! - Store data: prices and discounts for a region, and review counts (Steam's store API).
//! - Linux compatibility ratings from ProtonDB.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use serde::de::{DeserializeOwned, IgnoredAny};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::value::RawValue;

use crate::config;

const DAY: u64 = 24 * 3600;
pub const WEEK: u64 = 7 * DAY;

/// Requests made and bytes received by this process, for the usage shown in Settings.
static REQUESTS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);

/// Counts one answer. `sent` is the size the server reported (compressed), if it did;
/// otherwise the unpacked size is scaled by what gzip typically achieves: about 4:1 for web
/// pages and 7:1 for JSON (measured on Steam's answers).
pub(crate) fn count_text(sent: Option<u64>, text: &str) {
    let ratio = if text.trim_start().starts_with('<') { 4 } else { 7 };
    count(sent, text.len(), ratio);
}

fn count(sent: Option<u64>, unpacked: usize, ratio: u64) {
    let unpacked = unpacked as u64;
    let bytes = sent.filter(|&s| s > 0 && s <= unpacked).unwrap_or(if unpacked > 2048 { unpacked / ratio } else { unpacked });
    REQUESTS.fetch_add(1, Ordering::Relaxed);
    BYTES.fetch_add(bytes, Ordering::Relaxed);
}

pub(crate) fn sent_size(resp: &ureq::http::Response<ureq::Body>) -> Option<u64> {
    resp.headers().get("content-length")?.to_str().ok()?.parse().ok()
}

/// Spreads a periodic refresh over `days` by game, so games fetched together on the first run
/// don't all expire on the same day. Always the same offset for the same game.
pub fn spread(appid: u32, days: u64) -> u64 {
    // SplitMix64 scrambles the bits; app ids are often round numbers, which a plain
    // multiply would leave bunched on a few days.
    let mut x = u64::from(appid).wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    (x % days.max(1)) * DAY
}
pub(crate) const USER_AGENT: &str = concat!("Mozilla/5.0 (compatible; Depiled/", env!("CARGO_PKG_VERSION"), "; +https://github.com/Goggwell/depiled)");

/// One HTTP client for every source and every run. It keeps connections open between requests,
/// so a batch of pages to the same site costs one TLS handshake instead of one per page.
/// Answers arrive gzip-compressed (ureq asks for it and unpacks it). None of these sites
/// offers HTTP/2 or brotli to clients like this one, so gzip over HTTP/1.1 is what there is.
pub fn agent() -> ureq::Agent {
    static AGENT: std::sync::LazyLock<ureq::Agent> = std::sync::LazyLock::new(|| {
        ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(30)))
            .user_agent(USER_AGENT)
            // Error statuses come back as answers, so "too many requests" can be read for how
            // long to wait, and a 404 for what it means (see `Pace::send`).
            .http_status_as_error(false)
            .build()
            .new_agent()
    });
    AGENT.clone()
}

pub(crate) type Response = ureq::http::Response<ureq::Body>;

fn get(pace: &Pace, agent: &ureq::Agent, url: &str) -> Result<String> {
    let site = host(url);
    let mut resp = pace.send(site, || agent.get(url).call())?;
    ok(&resp, site)?;
    read_text(&mut resp, 8 * 1024 * 1024)
}

/// Reads an answer's text and counts it toward the data usage.
pub(crate) fn read_text(resp: &mut Response, limit: u64) -> Result<String> {
    let sent = sent_size(resp);
    let text = resp.body_mut().with_config().limit(limit).read_to_string().context("reading the answer")?;
    count_text(sent, &text);
    Ok(text)
}

/// An error for an answer that isn't a success (or "not modified").
pub(crate) fn ok(resp: &Response, site: &str) -> Result<()> {
    let status = resp.status();
    if status.is_success() || status == 304 { Ok(()) } else { Err(anyhow!("{site} answered {status}")) }
}

fn host(url: &str) -> &str {
    url.split('/').nth(2).unwrap_or(url)
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Keeps requests to one site at least `gap` apart, measured from the start of one request to
/// the start of the next. The time a response takes counts toward the gap, so a slow answer
/// isn't followed by a full pause as well: the site sees the same rate, the run finishes
/// sooner. Several threads can share one; they take turns.
pub(crate) struct Pace {
    slot: Mutex<(Duration, Option<Instant>)>,
}

/// Waits after a failure that is likely to pass, before the second and third tries.
const BACKOFF_SECS: [u64; 2] = [2, 8];
/// Longest wait a site can ask for before trying again.
const MAX_RETRY_AFTER_SECS: u64 = 60;

impl Pace {
    pub(crate) fn new(gap_ms: u64) -> Self {
        Self { slot: Mutex::new((Duration::from_millis(gap_ms), None)) }
    }

    /// Waits until the next request may start, then checks for a stop.
    pub(crate) fn wait(&self) -> Result<()> {
        {
            let mut slot = lock(&self.slot);
            let (gap, last) = *slot;
            if let Some(left) = last.and_then(|t| gap.checked_sub(t.elapsed())) {
                std::thread::sleep(left);
            }
            slot.1 = Some(Instant::now());
        }
        check_cancel()
    }

    /// The site asked to slow down: twice the gap for the rest of the run, up to 10 seconds.
    fn slow_down(&self) {
        let mut slot = lock(&self.slot);
        slot.0 = (slot.0 * 2).min(Duration::from_secs(10));
    }

    /// Waits for a turn and sends the request. A failure that is likely to pass (a timeout, a
    /// dropped connection, a server error, "too many requests") is tried again, up to three
    /// tries in all, after as long as the site asks or a short backoff. Every other answer is
    /// returned, error statuses included, for the caller to read.
    pub(crate) fn send(&self, site: &str, mut call: impl FnMut() -> Result<Response, ureq::Error>) -> Result<Response> {
        let mut backoff = BACKOFF_SECS.into_iter();
        loop {
            self.wait()?;
            let (asked, problem) = match call() {
                Ok(resp) => {
                    let status = resp.status();
                    if status.as_u16() != 429 && !status.is_server_error() {
                        return Ok(resp);
                    }
                    if status.as_u16() == 429 {
                        self.slow_down();
                    }
                    (retry_after(&resp), anyhow!("{site} answered {status}"))
                }
                Err(e) if transient(&e) => (None, anyhow!("couldn't reach {site}: {e}")),
                Err(e) => return Err(anyhow!("couldn't reach {site}: {e}")),
            };
            let Some(wait) = backoff.next() else { return Err(problem) };
            pause(Duration::from_secs(asked.unwrap_or(wait).min(MAX_RETRY_AFTER_SECS)))?;
        }
    }
}

/// Failures worth another try. Not "host not found": that's almost always being offline.
fn transient(e: &ureq::Error) -> bool {
    matches!(e, ureq::Error::Timeout(_) | ureq::Error::Io(_) | ureq::Error::ConnectionFailed | ureq::Error::BodyStalled | ureq::Error::Protocol(_))
}

/// Seconds the site asks to wait, if it says. (The date form isn't used by these sites.)
fn retry_after(resp: &Response) -> Option<u64> {
    resp.headers().get("retry-after")?.to_str().ok()?.trim().parse().ok()
}

/// Sleeps, waking now and then to stop if asked.
fn pause(d: Duration) -> Result<()> {
    let end = Instant::now() + d;
    while let Some(left) = end.checked_duration_since(Instant::now()).filter(|l| !l.is_zero()) {
        std::thread::sleep(left.min(Duration::from_millis(200)));
        check_cancel()?;
    }
    Ok(())
}

/// Reads a number however the API sends it: as a number or as text (Steam sends 64-bit
/// numbers as text). Anything else counts as unknown rather than failing the whole answer.
struct Lenient;

impl<'de> serde::de::Visitor<'de> for Lenient {
    type Value = Option<u64>;
    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("a number")
    }
    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E> {
        Ok(Some(v))
    }
    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E> {
        Ok(u64::try_from(v).ok())
    }
    fn visit_f64<E>(self, _: f64) -> Result<Self::Value, E> {
        Ok(None)
    }
    fn visit_bool<E>(self, _: bool) -> Result<Self::Value, E> {
        Ok(None)
    }
    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E> {
        Ok(v.trim().parse().ok())
    }
    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(None)
    }
    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(None)
    }
    fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        d.deserialize_any(self)
    }
    fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        while seq.next_element::<IgnoredAny>()?.is_some() {}
        Ok(None)
    }
    fn visit_map<A: serde::de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
        Ok(None)
    }
}

fn num<'de, D: Deserializer<'de>, T: TryFrom<u64> + Default>(d: D) -> Result<T, D::Error> {
    Ok(d.deserialize_any(Lenient)?.and_then(|n| T::try_from(n).ok()).unwrap_or_default())
}

fn opt_num<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u64>, D::Error> {
    d.deserialize_any(Lenient)
}

// ---- cache files ---------------------------------------------------------------------------

fn path(name: &str) -> Result<PathBuf> {
    Ok(config::dir()?.join(name))
}

pub fn load<T: DeserializeOwned + Default>(name: &str) -> T {
    path(name).ok().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub fn save<T: Serialize>(name: &str, value: &T) -> Result<()> {
    let path = path(name)?;
    std::fs::create_dir_all(path.parent().context("no parent")?)?;
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_vec(value)?)?;
    std::fs::rename(tmp, path)?;
    Ok(())
}

// ---- usage ---------------------------------------------------------------------------------

pub const USAGE_FILE: &str = "online-usage.json";

/// What the online extras downloaded, this month and in total.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Usage {
    /// Year and month the `month_*` counts are for, e.g. 2026-11.
    pub month: String,
    pub month_requests: u64,
    pub month_bytes: u64,
    pub total_requests: u64,
    pub total_bytes: u64,
    pub last_run: u64,
}

/// Year and month of a unix time, in UTC.
fn year_month(secs: u64) -> String {
    // Days to civil date (Howard Hinnant's algorithm).
    let z = (secs / DAY) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year}-{month:02}")
}

/// Adds what this process downloaded since the last call to the stored usage.
pub fn record_usage(now: u64) -> Result<Usage> {
    let requests = REQUESTS.swap(0, Ordering::Relaxed);
    let bytes = BYTES.swap(0, Ordering::Relaxed);
    let mut u: Usage = load(USAGE_FILE);
    let month = year_month(now);
    if u.month != month {
        u.month = month;
        u.month_requests = 0;
        u.month_bytes = 0;
    }
    u.month_requests += requests;
    u.month_bytes += bytes;
    u.total_requests += requests;
    u.total_bytes += bytes;
    u.last_run = now;
    save(USAGE_FILE, &u)?;
    Ok(u)
}

// ---- achievements from the public profile ---------------------------------------------------

pub const ACHIEVEMENTS_FILE: &str = "profile-achievements.json";

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Progress {
    pub unlocked: u32,
    pub total: u32,
    pub fetched: u64,
    /// Total playtime in minutes when fetched. Steam keeps the total across all computers,
    /// so a higher number later means the game was played somewhere since.
    #[serde(default)]
    pub playtime: u32,
    /// External achievement count already investigated on the last profile visit.
    #[serde(default)]
    pub compared_total: u32,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AchievementCache {
    #[serde(default)]
    pub games: BTreeMap<u32, Progress>,
    /// Set when the profile's game details aren't public, so nothing can be read.
    #[serde(default)]
    pub private: bool,
    #[serde(default)]
    pub checked: u64,
}

/// What Depiled knows about a game when deciding whether its achievement page is worth a
/// request.
#[derive(Debug, Clone, Copy, Default)]
pub struct AchievementHint {
    pub appid: u32,
    pub last_played: u64,
    pub playtime: u32,
    /// Total achievements according to another source (SteamHunters), 0 if unknown.
    pub known_total: u32,
    /// When Steam last wrote this game's progress to disk on this computer, 0 if never.
    pub local_at: u64,
    /// Steam's change number for the game, 0 if unknown. SteamHunters is asked again when it
    /// moves, since that's when an update can have added achievements.
    pub change: u32,
}

/// Games whose achievement page is worth fetching. Each page is the biggest download Depiled
/// makes, so a game is only fetched when something can have changed:
/// - never fetched, unless Steam already left fresh progress on disk;
/// - an update changed the number of achievements (SteamHunters, weekly);
/// - played since the last look, anywhere (total playtime grew), unless Steam has written
///   newer progress to disk since then, and the game isn't already at 100%;
/// - a slow catch-all every six to eight months, spread by game.
pub fn achievements_due(cache: &AchievementCache, hints: &[AchievementHint], now: u64, force: bool) -> Vec<AchievementHint> {
    hints
        .iter()
        .filter(|h| {
            let local_fresh = h.local_at > 0 && h.local_at >= h.last_played;
            match cache.games.get(&h.appid) {
                _ if force => true,
                None => !local_fresh,
                Some(p) => {
                    let total_changed = h.known_total > 0 && p.total > 0 && h.known_total != p.total && h.known_total != p.compared_total;
                    let complete = p.total > 0 && p.unlocked >= p.total;
                    // Entries saved before playtime was recorded have 0; judge those by date only.
                    let played = (p.playtime > 0 && h.playtime > p.playtime) || h.last_played > p.fetched;
                    let covered_locally = local_fresh && h.local_at > p.fetched;
                    total_changed || (!complete && played && !covered_locally) || now.saturating_sub(p.fetched) > 180 * DAY + spread(h.appid, 60)
                }
            }
        })
        .copied()
        .collect()
}

/// Set to stop a refresh between two requests.
pub static CANCEL: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(crate) fn check_cancel() -> Result<()> {
    if CANCEL.load(Ordering::Relaxed) {
        bail!("stopped");
    }
    Ok(())
}

/// Reads the unlocked and total counts out of a profile achievements page. `None` when the page
/// has no progress to show (private game details, or a game without stats).
///
/// The player's list holds every achievement, locked ones too, so an empty list means there was
/// nothing to read rather than nothing unlocked. Private game details look exactly like that.
pub fn parse_achievement_page(html: &str) -> Option<(u32, u32)> {
    let start = html.find("id=\"valve-ssr-data\"")?;
    let body = &html[start..];
    let body = &body[body.find('>')? + 1..];
    let body = &body[..body.find("</script>")?];
    // Only the few fields used are read; the rest of the page data (every achievement's name,
    // description and icon) is skipped over without building anything from it.
    let ssr: page::Ssr = serde_json::from_str(body).ok()?;
    let queries: page::Queries = serde_json::from_str(&ssr.render_context.query_data).ok()?;

    let mut unlocked = None;
    let mut total = None;
    for q in &queries.queries {
        let (Some(data), Some(kind)) = (q.state.as_ref().and_then(|s| s.data), q.query_key.first()) else { continue };
        if kind.get() != "\"achievements\"" {
            continue;
        }
        if q.query_key.get(2).map(|k| k.get()) == Some("\"user_achievements\"") {
            let player: page::Player = serde_json::from_str(data.get()).ok()?;
            if player.achievements.is_empty() {
                return None;
            }
            unlocked = Some(player.achievements.values().filter(|a| a.unlocked).count() as u32);
        } else if let Ok(schema) = serde_json::from_str::<page::Schema>(data.get()) {
            // The schema ("schema_v2"). Archived groups were removed from the game.
            total = Some(schema.groups.iter().filter(|g| !g.archived).map(|g| g.achievements.len() as u32).sum());
        }
    }
    Some((unlocked?, total?)).filter(|(_, t)| *t > 0)
}

/// The parts of a profile achievements page that are read.
mod page {
    use super::*;

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Ssr<'a> {
        #[serde(borrow)]
        pub render_context: RenderContext<'a>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct RenderContext<'a> {
        /// JSON inside a JSON string, so it's unescaped into its own text first.
        #[serde(borrow)]
        pub query_data: Cow<'a, str>,
    }

    #[derive(Deserialize)]
    pub struct Queries<'a> {
        #[serde(borrow)]
        pub queries: Vec<Query<'a>>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Query<'a> {
        #[serde(borrow, default)]
        pub query_key: Vec<&'a RawValue>,
        #[serde(borrow)]
        pub state: Option<State<'a>>,
    }

    #[derive(Deserialize)]
    pub struct State<'a> {
        #[serde(borrow)]
        pub data: Option<&'a RawValue>,
    }

    #[derive(Deserialize)]
    pub struct Player<'a> {
        #[serde(borrow)]
        pub achievements: BTreeMap<Cow<'a, str>, Unlocked>,
    }

    #[derive(Deserialize)]
    pub struct Unlocked {
        #[serde(default)]
        pub unlocked: bool,
    }

    #[derive(Deserialize)]
    pub struct Schema {
        pub groups: Vec<Group>,
    }

    #[derive(Deserialize)]
    pub struct Group {
        #[serde(default)]
        pub archived: bool,
        /// Only counted, so nothing is kept of each one.
        #[serde(default)]
        pub achievements: Vec<IgnoredAny>,
    }
}

/// Fetches achievement progress for `appids` from the user's public profile, one page per game.
pub fn refresh_achievements(steam64: u64, games: &[AchievementHint], now: u64, progress: &mut dyn FnMut(usize, usize)) -> Result<usize> {
    let mut cache: AchievementCache = load(ACHIEVEMENTS_FILE);
    let agent = agent();
    let mut misses = 0;
    let pace = Pace::new(1200);
    // Persist the partial batch on cancellation and on transport errors.
    let result = (|| -> Result<()> {
        for (i, hint) in games.iter().enumerate() {
            let AchievementHint { appid, playtime, known_total, .. } = *hint;
            // This address answers directly. The older /stats/<app>/achievements/ one redirects
            // twice first, tripling the round trips.
            let html = get(&pace, &agent, &format!("https://steamcommunity.com/profiles/{steam64}/achievements/{appid}?l=english"))?;
            match parse_achievement_page(&html) {
                Some((unlocked, total)) => {
                    cache.games.insert(appid, Progress { unlocked, total, fetched: now, playtime, compared_total: known_total });
                    cache.private = false;
                    misses = 0;
                }
                None => {
                    // Only games known to have achievements are asked for, so empty pages in a row
                    // mean the profile's game details are (now) private. Stop rather than keep asking.
                    misses += 1;
                    if misses >= 3 {
                        cache.private = true;
                        cache.checked = now;
                        bail!("your Steam profile's game details aren't public, so achievement progress can't be read");
                    }
                    // Remember the look so it isn't asked again until played; keep what was known.
                    let entry = cache.games.entry(appid).or_default();
                    entry.fetched = now;
                    entry.playtime = playtime;
                    entry.compared_total = known_total;
                }
            }
            if i % 5 == 4 {
                save(ACHIEVEMENTS_FILE, &cache)?;
            }
            progress(i + 1, games.len());
        }
        cache.checked = now;
        Ok(())
    })();
    save(ACHIEVEMENTS_FILE, &cache)?;
    result?;
    Ok(games.len())
}

// ---- store data: prices, discounts, reviews ------------------------------------------------

pub const STORE_FILE: &str = "store.json";

/// Empty and zero fields are left out of the file: most games aren't on sale, and the file
/// holds every game for every region visited, so this keeps it about a third smaller.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct StoreInfo {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// In the region's currency, in cents. `None` for free or unavailable items.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_cents: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_cents: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_text: Option<String>,
    #[serde(skip_serializing_if = "is_zero")]
    pub discount: u32,
    /// Unix seconds when the current sale ends, 0 if none.
    #[serde(skip_serializing_if = "is_zero")]
    pub discount_ends: u64,
    #[serde(skip_serializing_if = "is_zero")]
    pub review_count: u64,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub review_label: String,
    #[serde(skip_serializing_if = "is_zero")]
    pub review_percent: u32,
    #[serde(skip_serializing_if = "is_zero")]
    pub release: u64,
    pub fetched: u64,
}

fn is_zero<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
}

/// How many batches to fetch between saves of a cache file. The store file holds every region
/// visited, so rewriting it after each batch of 100 games would write megabytes for kilobytes
/// of news. A stop or an error still saves what was fetched.
const SAVE_EVERY: usize = 5;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct StoreCache {
    /// Two-letter country `apps` is for.
    #[serde(default)]
    pub country: String,
    #[serde(default)]
    pub apps: BTreeMap<u32, StoreInfo>,
    /// Data for regions used before, so switching back shows prices at once.
    #[serde(default)]
    pub others: BTreeMap<String, BTreeMap<u32, StoreInfo>>,
}

impl StoreCache {
    /// Makes `country` the current region, keeping the previous one's data aside.
    pub fn switch_to(&mut self, country: &str) {
        let country = country.to_ascii_uppercase();
        if self.country == country {
            return;
        }
        let previous = std::mem::take(&mut self.apps);
        if !self.country.is_empty() && !previous.is_empty() {
            self.others.insert(std::mem::take(&mut self.country), previous);
        }
        self.apps = self.others.remove(&country).unwrap_or_default();
        self.country = country;
    }

    fn for_country(&self, country: &str) -> Option<&BTreeMap<u32, StoreInfo>> {
        if self.country.eq_ignore_ascii_case(country) { Some(&self.apps) } else { self.others.get(&country.to_ascii_uppercase()) }
    }
}

/// Games whose store data is older than `max_age` (or missing) for `country`.
pub fn store_due(cache: &StoreCache, appids: &[u32], country: &str, now: u64, max_age: u64, force: bool) -> Vec<u32> {
    let known = cache.for_country(country);
    appids
        .iter()
        .copied()
        .filter(|a| force || known.and_then(|k| k.get(a)).is_none_or(|s| now.saturating_sub(s.fetched) >= max_age))
        .collect()
}

/// The parts of the store's answer that are read.
mod items {
    use super::*;

    #[derive(Deserialize)]
    pub struct Answer {
        pub response: Option<Body>,
    }

    #[derive(Deserialize)]
    pub struct Body {
        pub store_items: Option<Vec<Item>>,
        /// Anything else, to tell an answer with no items from one with something unexpected.
        #[serde(flatten)]
        pub rest: BTreeMap<String, IgnoredAny>,
    }

    #[derive(Deserialize)]
    pub struct Item {
        #[serde(default, deserialize_with = "opt_num")]
        pub appid: Option<u64>,
        #[serde(default, deserialize_with = "opt_num")]
        pub id: Option<u64>,
        #[serde(default)]
        pub name: String,
        pub best_purchase_option: Option<Purchase>,
        pub reviews: Option<Reviews>,
        pub release: Option<Release>,
    }

    #[derive(Deserialize)]
    pub struct Purchase {
        #[serde(default, deserialize_with = "opt_num")]
        pub final_price_in_cents: Option<u64>,
        #[serde(default, deserialize_with = "opt_num")]
        pub original_price_in_cents: Option<u64>,
        pub formatted_final_price: Option<String>,
        #[serde(default, deserialize_with = "num")]
        pub discount_pct: u32,
        #[serde(default)]
        pub active_discounts: Vec<Discount>,
    }

    #[derive(Deserialize)]
    pub struct Discount {
        #[serde(default, deserialize_with = "num")]
        pub discount_end_date: u64,
    }

    #[derive(Deserialize)]
    pub struct Reviews {
        pub summary_filtered: Option<Summary>,
    }

    #[derive(Deserialize)]
    pub struct Summary {
        #[serde(default, deserialize_with = "num")]
        pub review_count: u64,
        #[serde(default, deserialize_with = "num")]
        pub percent_positive: u32,
        #[serde(default)]
        pub review_score_label: String,
    }

    #[derive(Deserialize)]
    pub struct Release {
        #[serde(default, deserialize_with = "num")]
        pub steam_release_date: u64,
    }
}

pub fn parse_store_items(text: &str, now: u64) -> Result<Vec<(u32, StoreInfo)>> {
    let answer: items::Answer = serde_json::from_str(text).context("the store sent something unexpected")?;
    let body = answer.response.context("the store response is missing")?;
    let items = match body.store_items {
        Some(items) => items,
        None if body.rest.is_empty() => Vec::new(),
        None => bail!("the store response has no items"),
    };
    Ok(items
        .into_iter()
        .filter_map(|it| {
            let appid = u32::try_from(it.appid.or(it.id)?).ok()?;
            let best = it.best_purchase_option;
            let reviews = it.reviews.and_then(|r| r.summary_filtered);
            Some((
                appid,
                StoreInfo {
                    name: it.name,
                    price_cents: best.as_ref().and_then(|b| b.final_price_in_cents),
                    original_cents: best.as_ref().and_then(|b| b.original_price_in_cents),
                    discount: best.as_ref().map_or(0, |b| b.discount_pct),
                    discount_ends: best.as_ref().and_then(|b| b.active_discounts.first()).map_or(0, |d| d.discount_end_date),
                    price_text: best.and_then(|b| b.formatted_final_price),
                    review_count: reviews.as_ref().map_or(0, |r| r.review_count),
                    review_percent: reviews.as_ref().map_or(0, |r| r.percent_positive),
                    review_label: reviews.map(|r| r.review_score_label).unwrap_or_default(),
                    release: it.release.map_or(0, |r| r.steam_release_date),
                    fetched: now,
                },
            ))
        })
        .collect())
}

/// What the store is asked for: the items, the region, and reviews and release dates with them.
#[derive(Serialize)]
struct ItemsQuery<'a> {
    ids: Vec<ItemId>,
    context: ItemsContext<'a>,
    data_request: ItemsRequest,
}

#[derive(Serialize)]
struct ItemId {
    appid: u32,
}

#[derive(Serialize)]
struct ItemsContext<'a> {
    language: &'a str,
    country_code: &'a str,
}

#[derive(Serialize)]
struct ItemsRequest {
    include_reviews: bool,
    include_release: bool,
}

fn apply_store_batch(cache: &mut StoreCache, batch: &[u32], text: &str, now: u64) -> Result<()> {
    for (appid, info) in parse_store_items(text, now)? {
        cache.apps.insert(appid, info);
    }
    // Omitted items still count as checked, including ones with older cached details.
    for appid in batch {
        cache.apps.entry(*appid).or_default().fetched = now;
    }
    Ok(())
}

/// Looks up the requested app IDs in batches of 100.
pub fn refresh_store(appids: &[u32], country: &str, now: u64, progress: &mut dyn FnMut(usize, usize)) -> Result<usize> {
    let mut cache: StoreCache = load(STORE_FILE);
    cache.switch_to(country);
    let agent = agent();
    let pace = Pace::new(1000);
    let mut done = 0;
    let result = (|| -> Result<()> {
        for (i, batch) in appids.chunks(100).enumerate() {
            // The best purchase option (price, sale and its end date) comes with any request; asking
            // for every purchase option as well would triple the size for nothing Depiled uses.
            let input = serde_json::to_string(&ItemsQuery {
                ids: batch.iter().map(|&appid| ItemId { appid }).collect(),
                context: ItemsContext { language: "english", country_code: &cache.country },
                data_request: ItemsRequest { include_reviews: true, include_release: true },
            })?;
            let mut resp = pace.send("the Steam store", || agent.get("https://api.steampowered.com/IStoreBrowseService/GetItems/v1/").query("input_json", &input).call())?;
            ok(&resp, "the Steam store")?;
            let text = read_text(&mut resp, 16 * 1024 * 1024)?;
            apply_store_batch(&mut cache, batch, &text, now)?;
            if i % SAVE_EVERY == SAVE_EVERY - 1 {
                save(STORE_FILE, &cache)?;
            }
            done += batch.len();
            progress(done, appids.len());
        }
        Ok(())
    })();
    save(STORE_FILE, &cache)?;
    result?;
    Ok(done)
}

// ---- ProtonDB ------------------------------------------------------------------------------

pub const PROTON_FILE: &str = "protondb.json";

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Proton {
    /// platinum, gold, silver, bronze, borked; empty when ProtonDB has no reports.
    pub tier: String,
    pub reports: u32,
    pub fetched: u64,
    /// The answer's version tag. Sent back on the next check, so an unchanged rating comes back
    /// as "not modified" with no body.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub etag: String,
}

pub fn proton_due(cache: &BTreeMap<u32, Proton>, appids: &[u32], now: u64, force: bool) -> Vec<u32> {
    appids
        .iter()
        .copied()
        .filter(|a| {
            force
                || cache.get(a).is_none_or(|p| {
                    // Well-established ratings rarely move, so they're checked less often, and
                    // so are games nobody has reported on yet: most of them never get a report.
                    let settled = matches!(p.tier.as_str(), "platinum" | "gold" | "borked") && p.reports >= 50;
                    let every = if settled { 90 } else if p.tier.is_empty() { 60 } else { 30 };
                    now.saturating_sub(p.fetched) >= every * DAY + spread(*a, every / 2 + 1)
                })
        })
        .collect()
}

/// Ratings are looked up this many at a time. Each is a tiny file from ProtonDB's CDN, so the
/// time goes on the round trip, not the transfer; starts are still spaced by the shared pace.
const PROTON_AT_ONCE: usize = 4;
/// Least time between the starts of two lookups, across all of them.
const PROTON_GAP_MS: u64 = 150;

#[derive(Deserialize)]
struct ProtonSummary {
    tier: String,
    #[serde(default, deserialize_with = "num")]
    total: u32,
}

fn fetch_proton(pace: &Pace, agent: &ureq::Agent, appid: u32, known: Option<Proton>, now: u64) -> Result<Proton> {
    let url = format!("https://www.protondb.com/api/v1/reports/summaries/{appid}.json");
    let mut resp = pace.send("ProtonDB", || {
        let mut req = agent.get(&url);
        if let Some(p) = &known {
            req = req.header("If-None-Match", &p.etag);
        }
        req.call()
    })?;
    Ok(match resp.status().as_u16() {
        304 => {
            count(None, 0, 1);
            Proton { fetched: now, ..known.unwrap_or_default() }
        }
        // No reports for this game.
        404 => {
            count(None, 0, 1);
            Proton { fetched: now, ..Proton::default() }
        }
        _ => {
            ok(&resp, "ProtonDB")?;
            let etag = resp.headers().get("etag").and_then(|v| v.to_str().ok()).unwrap_or_default().to_owned();
            let text = read_text(&mut resp, 1024 * 1024)?;
            let summary: ProtonSummary = serde_json::from_str(&text).context("ProtonDB sent something unexpected")?;
            Proton { tier: summary.tier, reports: summary.total, fetched: now, etag }
        }
    })
}

pub fn refresh_proton(appids: &[u32], now: u64, progress: &mut (dyn FnMut(usize, usize) + Send)) -> Result<usize> {
    let cache: Mutex<BTreeMap<u32, Proton>> = Mutex::new(load(PROTON_FILE));
    let agent = agent();
    let pace = Pace::new(PROTON_GAP_MS);
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let failed: Mutex<Option<anyhow::Error>> = Mutex::new(None);
    let progress = Mutex::new(progress);
    std::thread::scope(|scope| {
        for _ in 0..PROTON_AT_ONCE.min(appids.len()) {
            scope.spawn(|| {
                // Each worker takes the next game until none are left, or one fails or is
                // stopped, which ends the others after their lookup in flight.
                while lock(&failed).is_none() {
                    let Some(&appid) = appids.get(next.fetch_add(1, Ordering::Relaxed)) else { break };
                    let known = lock(&cache).get(&appid).filter(|p| !p.etag.is_empty()).cloned();
                    let entry = match fetch_proton(&pace, &agent, appid, known, now) {
                        Ok(entry) => entry,
                        Err(e) => {
                            lock(&failed).get_or_insert(e);
                            break;
                        }
                    };
                    let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                    {
                        let mut cache = lock(&cache);
                        cache.insert(appid, entry);
                        // Saved as it goes, so a stop or a crash keeps what was fetched.
                        if n.is_multiple_of(25)
                            && let Err(e) = save(PROTON_FILE, &*cache)
                        {
                            lock(&failed).get_or_insert(e);
                        }
                    }
                    (*lock(&progress))(n, appids.len());
                }
            });
        }
    });
    save(PROTON_FILE, &cache.into_inner().unwrap_or_else(PoisonError::into_inner))?;
    if let Some(e) = failed.into_inner().unwrap_or_else(PoisonError::into_inner) {
        return Err(e);
    }
    Ok(done.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_progress_from_the_profile_page() {
        let queries = serde_json::json!({ "queries": [
            { "queryKey": ["achievements", 40800, "user_achievements", "1"], "state": { "data": { "achievements": {
                "256": { "unlocked": true }, "263": { "unlocked": true }, "300": { "unlocked": false } } } } },
            { "queryKey": ["achievements", 40800, "schema", "english"], "state": { "data": { "groups": [
                { "achievements": [{}, {}, {}, {}] }, { "achievements": [{}] } ] } } }
        ]});
        let ssr = serde_json::json!({ "renderContext": { "queryData": queries.to_string() } });
        let html = format!("<html><script type=\"application/json\" id=\"valve-ssr-data\" nonce=\"x\">{ssr}</script></html>");
        assert_eq!(parse_achievement_page(&html), Some((2, 5)));
        assert_eq!(parse_achievement_page("<html>private</html>"), None);

        // Private game details: the page still renders, with an empty list for the player.
        let hidden = serde_json::json!({ "queries": [
            { "queryKey": ["achievements", 40800, "user_achievements", "1"], "state": { "data": { "achievements": {}, "groups": {}, "schema_hash": 0 } } },
            { "queryKey": ["achievements", 40800, "schema_v2"], "state": { "data": { "groups": [
                { "archived": false, "achievements": [{}, {}] }, { "archived": true, "achievements": [{}] } ] } } }
        ]});
        let ssr = serde_json::json!({ "renderContext": { "queryData": hidden.to_string() } });
        let html = format!("<script id=\"valve-ssr-data\">{ssr}</script>");
        assert_eq!(parse_achievement_page(&html), None, "not 0 of 2");
    }

    #[test]
    fn achievement_pages_are_only_fetched_when_something_can_have_changed() {
        let p = |unlocked, total| Progress { unlocked, total, fetched: 1_000, playtime: 60, compared_total: 0 };
        let mut cache = AchievementCache::default();
        cache.games.insert(1, p(5, 10));
        cache.games.insert(2, p(5, 10));
        cache.games.insert(3, p(10, 10));
        cache.games.insert(4, p(10, 10));
        cache.games.insert(5, p(5, 10));
        cache.games.insert(6, p(5, 10));
        let h = |appid, last_played, playtime, known_total, local_at| AchievementHint { appid, last_played, playtime, known_total, local_at, change: 0 };
        let hints = [
            h(1, 500, 60, 10, 0),       // nothing new
            h(2, 500, 90, 10, 0),       // played on another computer: playtime grew
            h(3, 2_000, 90, 10, 0),     // played, but already at 100%: can't change
            h(4, 500, 60, 12, 0),       // an update added two achievements
            h(5, 2_000, 90, 10, 2_100), // played here, and Steam wrote fresh progress to disk
            h(6, 2_000, 90, 10, 0),     // played here, nothing on disk
            h(7, 500, 60, 10, 600),     // never fetched, fresh progress on disk already
            h(8, 500, 60, 10, 0),       // never fetched, nothing on disk
        ];
        let due_ids = |c: &AchievementCache| achievements_due(c, &hints, 3_000, false).into_iter().map(|h| h.appid).collect::<Vec<_>>();
        assert_eq!(due_ids(&cache), vec![2, 4, 6, 8]);
        cache.games.get_mut(&4).unwrap().compared_total = 12;
        assert_eq!(due_ids(&cache), vec![2, 6, 8]);
        assert_eq!(achievements_due(&cache, &hints, 3_000, true).len(), 8);
    }

    #[test]
    fn parses_store_items() {
        let text = r#"{"response":{"store_items":[{"appid":620,"name":"Portal 2","best_purchase_option":{"final_price_in_cents":"519","original_price_in_cents":"2599","formatted_final_price":"5,19€","discount_pct":80,"active_discounts":[{"discount_end_date":1796061600}]},"reviews":{"summary_filtered":{"review_count":393452,"percent_positive":98,"review_score_label":"Overwhelmingly Positive"}},"release":{"steam_release_date":1303171200}}]}}"#;
        let items = parse_store_items(text, 7).unwrap();
        let (id, s) = &items[0];
        assert_eq!((*id, s.price_cents, s.original_cents, s.discount, s.discount_ends), (620, Some(519), Some(2599), 80, 1_796_061_600));
        assert_eq!((s.review_count, s.review_label.as_str(), s.release, s.fetched), (393_452, "Overwhelmingly Positive", 1_303_171_200, 7));
    }

    #[test]
    fn store_refreshes_by_age_and_region() {
        let mut cache = StoreCache { country: "NL".into(), ..StoreCache::default() };
        cache.apps.insert(1, StoreInfo { fetched: 0, ..StoreInfo::default() });
        cache.apps.insert(2, StoreInfo { fetched: DAY, ..StoreInfo::default() });
        assert_eq!(store_due(&cache, &[1, 2, 3], "nl", DAY + 10, DAY, false), vec![1, 3]);
        assert_eq!(store_due(&cache, &[1, 2], "nl", DAY + 10, WEEK, false), Vec::<u32>::new());
        assert_eq!(store_due(&cache, &[1, 2], "US", DAY + 10, DAY, false), vec![1, 2]);
    }

    #[test]
    fn omitted_store_items_wait_until_due_again() {
        let mut cache = StoreCache { country: "US".into(), ..StoreCache::default() };
        cache.apps.insert(10, StoreInfo { name: "Unavailable".into(), fetched: 1, ..StoreInfo::default() });
        let now = 2 * WEEK;
        apply_store_batch(&mut cache, &[10, 20], r#"{"response":{"store_items":[]}}"#, now).unwrap();
        assert_eq!(cache.apps[&10].name, "Unavailable");
        assert!(store_due(&cache, &[10, 20], "US", now + 1, WEEK, false).is_empty());
        assert_eq!(store_due(&cache, &[10, 20], "US", now + WEEK, WEEK, false), vec![10, 20]);
        assert!(apply_store_batch(&mut cache, &[10], "{}", now + WEEK).is_err());
        assert_eq!(cache.apps[&10].fetched, now);
    }

    #[test]
    fn switching_region_keeps_the_previous_one() {
        let mut cache = StoreCache { country: "NL".into(), ..StoreCache::default() };
        cache.apps.insert(1, StoreInfo { price_cents: Some(999), fetched: 5, ..StoreInfo::default() });
        cache.switch_to("us");
        assert_eq!((cache.country.as_str(), cache.apps.len()), ("US", 0));
        cache.apps.insert(1, StoreInfo { price_cents: Some(1099), fetched: 6, ..StoreInfo::default() });
        cache.switch_to("NL");
        assert_eq!(cache.apps[&1].price_cents, Some(999), "Dutch prices come straight back");
        assert_eq!(store_due(&cache, &[1], "US", 10, DAY, false), Vec::<u32>::new(), "and the US ones are still fresh");
    }

    #[test]
    fn pacing_counts_the_time_a_request_took() {
        let pace = Pace::new(80);
        pace.wait().unwrap();
        std::thread::sleep(Duration::from_millis(100)); // a slow answer, longer than the gap
        let t = std::time::Instant::now();
        pace.wait().unwrap();
        assert!(t.elapsed() < Duration::from_millis(20), "no extra pause after a slow answer");
        let t = std::time::Instant::now();
        pace.wait().unwrap();
        assert!(t.elapsed() >= Duration::from_millis(70), "back-to-back requests are still spaced");
    }

    fn answer(status: u16, retry_after: Option<&str>) -> Result<Response, ureq::Error> {
        let mut b = ureq::http::Response::builder().status(status);
        if let Some(s) = retry_after {
            b = b.header("retry-after", s);
        }
        Ok(b.body(ureq::Body::builder().data("")).unwrap())
    }

    #[test]
    fn passing_failures_are_tried_again_and_others_are_not() {
        let pace = Pace::new(0);
        // Too many requests, then a server error, then it works: three tries.
        let mut tries = 0;
        let replies = [429, 502, 200];
        let resp = pace.send("site", || {
            tries += 1;
            answer(replies[tries - 1], Some("0"))
        });
        assert_eq!((resp.unwrap().status().as_u16(), tries), (200, 3));
        assert_eq!(lock(&pace.slot).0, Duration::ZERO, "doubling nothing is still nothing");

        // A 404 is an answer, not something to retry.
        let mut tries = 0;
        let resp = pace.send("site", || {
            tries += 1;
            answer(404, None)
        });
        assert_eq!((resp.unwrap().status().as_u16(), tries), (404, 1));

        // Three failures in a row give up with the last one.
        let mut tries = 0;
        let err = pace.send("site", || {
            tries += 1;
            answer(503, Some("0"))
        });
        assert_eq!(tries, 3);
        assert!(err.unwrap_err().to_string().contains("503"));
    }

    #[test]
    fn too_many_requests_slows_the_rest_of_the_run() {
        let pace = Pace::new(100);
        let mut first = true;
        pace.send("site", || if std::mem::take(&mut first) { answer(429, Some("0")) } else { answer(200, None) }).unwrap();
        assert_eq!(lock(&pace.slot).0, Duration::from_millis(200));
    }

    #[test]
    fn numbers_are_read_however_they_are_sent() {
        let text = r#"{"response":{"store_items":[{"appid":"620","best_purchase_option":{"final_price_in_cents":999,"discount_pct":"25","active_discounts":[{"discount_end_date":{}}]},"reviews":{"summary_filtered":{"review_count":-1}}}]}}"#;
        let items = parse_store_items(text, 1).unwrap();
        let (appid, info) = &items[0];
        assert_eq!((*appid, info.price_cents, info.discount, info.discount_ends, info.review_count), (620, Some(999), 25, 0, 0));
        assert!(parse_store_items(r#"{"response":{}}"#, 1).unwrap().is_empty());
        assert!(parse_store_items(r#"{"response":{"error":"busy"}}"#, 1).is_err());
        assert!(parse_store_items(r#"{}"#, 1).is_err());
    }

    #[test]
    fn periodic_refreshes_are_spread_out() {
        let days: std::collections::BTreeSet<u64> = (0..200u32).map(|a| spread(a * 10, 30) / DAY).collect();
        assert!(days.len() > 20, "200 games land on most of the 30 days");
        assert_eq!(spread(620, 30), spread(620, 30));
        assert_eq!(year_month(1_795_670_000), "2026-11");
    }
}
