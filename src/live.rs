use anyhow::{Result, bail};
use serde_json::{Value, json};

use crate::cdp;
use crate::model::{Achievements, Collection, Game};
use crate::ops::Op;

/// A connection to the running Steam client's library context.
pub struct Live {
    ws_url: String,
}

const READ_JS: &str = r#"(() => collectionStore.userCollections
  .filter(c => c.m_strId.startsWith('uc-'))
  .map(c => ({
    id: c.m_strId,
    name: c.displayName,
    apps: [...c.m_setAddedManually],
    dynamic: !!c.bIsDynamic || !c.bIsEditable,
  })))()"#;

/// Library games only, with the fields the rules use. Steam's "my games" list also holds
/// demos (app_type 8) and playtests (65536); only app_type 1 is a game.
const GAMES_JS: &str = r#"(() => collectionStore.myGamesCollection.allApps.filter(a => a.app_type === 1).map(a => ({
  appid: a.appid,
  name: a.display_name,
  playtime_minutes: a.minutes_playtime_forever || 0,
  last_played: a.rt_last_time_played || 0,
  categories: Array.from(a.m_setStoreCategories || []),
  tags: Array.from(a.m_setStoreTags || []),
  metacritic: a.metacritic_score || 0,
  review_percent: a.review_percentage_without_bombs || 0,
})))()"#;

/// Asks Steam's own achievement-progress cache, which the client fills over its normal
/// connection, so no Web API key or public profile is needed. Entries older than the last
/// play session are refreshed first.
const ACHIEVEMENTS_JS: &str = r#"
const c = appAchievementProgressCache;
const cache = () => c.m_achievementProgress && c.m_achievementProgress.mapCache;
const seen = {};
for (const id of args.ids) {
  const e = cache() && cache().get(id);
  seen[id] = e ? e.cache_time : -1;
  c.GetAchievementProgress(id);
}
const o = id => appStore.GetAppOverviewByAppID(id);
const pending = () => args.ids.filter(id => {
  const e = cache() && cache().get(id);
  if (!e) return true;
  return o(id) && o(id).rt_last_time_played > seen[id] && e.cache_time <= seen[id];
});
const deadline = Date.now() + args.timeoutMs;
while (pending().length && Date.now() < deadline) await new Promise(r => setTimeout(r, 400));
const out = {};
for (const id of args.ids) {
  const e = cache() && cache().get(id);
  if (e) out[id] = { unlocked: e.unlocked, total: e.total };
}
return out;
"#;

/// A checksum over every game's playtime and last-played time, worked out inside Steam. The
/// watcher asks this every few seconds, so it must stay cheap however big the library is:
/// the answer is two numbers, not the whole game list.
const FINGERPRINT_JS: &str = r#"(() => {
  let h = 0, n = 0;
  for (const a of collectionStore.myGamesCollection.allApps) {
    if (a.app_type !== 1) continue;
    h = (Math.imul(h ^ a.appid, 16777619) + (a.minutes_playtime_forever | 0) * 31 + (a.rt_last_time_played | 0)) | 0;
    n++;
  }
  return [h, n];
})()"#;

impl Live {
    /// Changes whenever any game's playtime or last-played time changes.
    pub fn fingerprint(&self) -> Result<u64> {
        let parts: Vec<i64> = serde_json::from_value(cdp::eval(&self.ws_url, FINGERPRINT_JS)?)?;
        match parts.as_slice() {
            [hash, count] => Ok(((*hash as u32 as u64) << 32) | (*count as u32 as u64)),
            _ => bail!("unexpected fingerprint reply"),
        }
    }

    /// Achievement counts for the given games. Games Steam hasn't answered for in time are
    /// left out, and the rules treat unknown progress as not complete.
    pub fn achievements(&self, appids: &[u32]) -> Result<std::collections::BTreeMap<u32, Achievements>> {
        let args = json!({ "ids": appids, "timeoutMs": 20_000 });
        let expression = format!("(async (args) => {{ {ACHIEVEMENTS_JS} }})({args})");
        let raw: std::collections::BTreeMap<String, Achievements> =
            serde_json::from_value(cdp::eval(&self.ws_url, &expression)?)?;
        Ok(raw.into_iter().filter_map(|(id, a)| Some((id.parse().ok()?, a))).collect())
    }

    /// Steam's own tag and genre names, so rules can say "Horror" instead of 1667.
    pub fn tag_names(&self) -> Result<std::collections::BTreeMap<u32, String>> {
        let value = cdp::eval(&self.ws_url, "(() => ({ ...appStore.m_mapStoreTagLocalization }))()")?;
        let raw: std::collections::BTreeMap<String, String> = serde_json::from_value(value)?;
        Ok(raw.into_iter().filter_map(|(id, name)| Some((id.parse().ok()?, name))).collect())
    }

    pub fn games(&self) -> Result<Vec<Game>> {
        Ok(serde_json::from_value(cdp::eval(&self.ws_url, GAMES_JS)?)?)
    }

    pub fn connect(port: u16) -> Result<Self> {
        Ok(Self { ws_url: cdp::shared_context_url(port)? })
    }

    pub fn collections(&self) -> Result<Vec<Collection>> {
        let value = cdp::eval(&self.ws_url, READ_JS)?;
        Ok(serde_json::from_value(value)?)
    }

    /// Applies one change through Steam's own collection store, then waits for Steam to
    /// finish its save so the next operation sees the result.
    pub fn apply(&self, op: &Op) -> Result<()> {
        let (js, args) = match op {
            Op::Create { name, apps } => (CREATE_JS, json!({ "name": name, "apps": apps })),
            Op::Add { id, apps, .. } => (MEMBERS_JS, json!({ "id": id, "apps": apps, "add": true })),
            Op::Remove { id, apps, .. } => (MEMBERS_JS, json!({ "id": id, "apps": apps, "add": false })),
            Op::Rename { id, to, .. } => (RENAME_JS, json!({ "id": id, "name": to })),
            Op::Delete { id, .. } => (DELETE_JS, json!({ "id": id })),
        };
        let expression = format!("(async (args) => {{ {js} }})({args})");
        let result = cdp::eval(&self.ws_url, &expression)?;
        if let Some(err) = result.get("error").and_then(Value::as_str) {
            bail!("{op}: {err}");
        }
        Ok(())
    }
}

const CREATE_JS: &str = concat!(
    "const c = collectionStore.NewUnsavedCollection(args.name, undefined, []); await c.Save();",
    "const made = collectionStore.userCollections.find(x => x.displayName === args.name);",
    "if (!made) return { error: 'collection missing after save' };",
    "if (args.apps.length) collectionStore.AddOrRemoveApp(args.apps, true, made.m_strId);",
    "await new Promise(r => setTimeout(r, 800)); return { id: made.m_strId };"
);

const MEMBERS_JS: &str = concat!(
    "if (!collectionStore.GetCollection(args.id)) return { error: 'collection not found' };",
    "collectionStore.AddOrRemoveApp(args.apps, args.add, args.id);",
    "await new Promise(r => setTimeout(r, 800)); return { ok: true };"
);

const DELETE_JS: &str = concat!(
    "const c = collectionStore.GetCollection(args.id);",
    "if (!c || !c.bIsDeletable) return { error: 'collection not found or not deletable' };",
    "await c.Delete(); await new Promise(r => setTimeout(r, 800)); return { ok: true };"
);

const RENAME_JS: &str = concat!(
    "const c = collectionStore.GetCollection(args.id);",
    "if (!c) return { error: 'collection not found' };",
    "c.m_strName = args.name; await c.Save();",
    "await new Promise(r => setTimeout(r, 800)); return { name: c.displayName };"
);
