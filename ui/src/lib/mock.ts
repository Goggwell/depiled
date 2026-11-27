// Demo backend used when the page runs in a normal browser, so the design can be worked on
// without Steam. It holds invented games in memory and behaves like the real commands,
// including the draft: edits stay unsaved until "save".
import type { Collection, DraftState, Game, Kind, Library, StoreInfo, OpView, Patch, SmartRule, SnapshotInfo } from './api';
import { matchesRule } from './criteria';

const WORDS_A = ['Neon', 'Hollow', 'Crimson', 'Quiet', 'Iron', 'Lunar', 'Rusted', 'Velvet', 'Ember', 'Glass', 'Wild', 'Paper', 'Distant', 'Silent', 'Golden'];
const WORDS_B = ['Knight', 'Garden', 'Signal', 'Frontier', 'Harbor', 'Circuit', 'Tide', 'Citadel', 'Orchard', 'Descent', 'Odyssey', 'Protocol', 'Kingdom', 'Reverie', 'Outpost'];
const STUDIOS = ['Lantern Works', 'Moss & Co', 'Tidewater', 'Northlight', 'Small Hours'];
const TAGS: Record<string, string> = {
  '19': 'Action', '21': 'Adventure', '122': 'RPG', '9': 'Strategy', '597': 'Casual', '1667': 'Horror', '1716': 'Roguelike',
  '1742': 'Story Rich', '3942': 'Cozy', '1664': 'Puzzle', '1685': 'Co-op', '4182': 'Singleplayer', '5395': '3D Platformer', '1695': 'Open World',
};
const FLAGS = [
  { id: 2, name: 'Single-player' }, { id: 1, name: 'Multi-player' }, { id: 9, name: 'Co-op' },
  { id: 23, name: 'Steam Cloud' }, { id: 28, name: 'Full controller support' }, { id: 22, name: 'Steam Achievements' }, { id: 29, name: 'Steam Trading Cards' },
];

let seed = 7;
const rnd = () => (seed = (seed * 16807) % 2147483647) / 2147483647;
const pick = <T,>(xs: T[]) => xs[Math.floor(rnd() * xs.length)];
const now = Math.floor(Date.now() / 1000);
const DAY = 86400;

/** Invented store data: a price, sometimes on sale, and review counts. */
function demoStore(i: number): StoreInfo {
  const price = [499, 999, 1499, 1999, 2999, 5999][i % 6];
  const discount = i % 5 === 0 ? [20, 50, 75, 90][i % 4] : 0;
  const final = Math.round(price * (1 - discount / 100));
  const count = Math.floor(200 + ((i * 7919) % 90000));
  return {
    price_cents: final,
    price_text: `$${(final / 100).toFixed(2)}`,
    discount,
    discount_ends: discount ? Math.floor(Date.now() / 1000) + 5 * 86400 : 0,
    review_count: count,
    review_label: count > 50000 ? 'Overwhelmingly Positive' : 'Very Positive',
    fetched: Math.floor(Date.now() / 1000) - 3600,
  };
}

/** Invented box art: a few bands of colour, so the demo looks like a real shelf. */
function demoCover(i: number): string {
  const h = (i * 67) % 360;
  const sky = `hsl(${h} 45% ${30 + (i % 4) * 8}%)`;
  const land = `hsl(${(h + 40) % 360} 35% ${18 + (i % 3) * 6}%)`;
  const sun = `hsl(${(h + 180) % 360} 70% 70%)`;
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 300"><rect width="200" height="300" fill="${sky}"/><circle cx="${50 + (i % 5) * 25}" cy="${70 + (i % 3) * 20}" r="${18 + (i % 4) * 6}" fill="${sun}" opacity=".85"/><path d="M0 ${190 + (i % 4) * 10} L${60 + (i % 6) * 10} ${150 + (i % 5) * 8} L130 ${200 - (i % 3) * 12} L200 ${160 + (i % 4) * 9} V300 H0Z" fill="${land}"/></svg>`;
  return `data:image/svg+xml,${encodeURIComponent(svg)}`;
}

/** `?games=10000` in the page address makes a bigger invented library, to test performance. */
const COUNT = Number(new URLSearchParams(globalThis.location?.search ?? '').get('games')) || 140;

const games: Game[] = Array.from({ length: COUNT }, (_, i) => {
  const never = rnd() < 0.35;
  const minutes = never ? 0 : Math.floor(rnd() ** 2 * 6000) + 15;
  const total = rnd() < 0.8 ? 10 + Math.floor(rnd() * 50) : 0;
  const median = total ? Math.floor(300 + rnd() ** 2 * 6000) : 0;
  return {
    appid: 1000 + i,
    kind: (i % 17 === 3 ? 'demo' : i % 31 === 7 ? 'software' : 'game') as Kind,
    name: `${pick(WORDS_A)} ${pick(WORDS_B)}${rnd() < 0.2 ? ' II' : ''}${COUNT > 400 ? ` ${i}` : ''}`,
    playtime_minutes: minutes,
    last_played: never ? 0 : now - Math.floor(rnd() * 120) * DAY,
    categories: FLAGS.filter(() => rnd() < 0.5).map((f) => f.id),
    tags: Object.keys(TAGS).filter(() => rnd() < 0.22).map(Number),
    metacritic: rnd() < 0.5 ? Math.floor(60 + rnd() * 38) : 0,
    review_percent: Math.floor(55 + rnd() * 45),
    release: now - Math.floor(rnd() * 4000) * DAY,
    developers: [pick(STUDIOS)],
    publishers: [pick(STUDIOS)],
    achievements: !never && total && rnd() < 0.6 ? { total, unlocked: rnd() < 0.2 ? total : Math.floor(total * rnd()) } : null,
    hunters: total ? { median_minutes: median, fastest_minutes: Math.floor(median / (3 + rnd() * 5)), perfected: Math.floor(rnd() * 20000), players: 20000 + Math.floor(rnd() * 80000), achievements: total } : null,
    extras: { deck: Math.floor(rnd() * 4), controller: Math.floor(rnd() * 3), platforms: 1 | (rnd() < 0.3 ? 4 : 0) | (rnd() < 0.2 ? 2 : 0), vr: rnd() < 0.05, early_access: rnd() < 0.08 },
    installed_bytes: rnd() < 0.15 ? Math.floor(rnd() * 60e9) : 0,
    download_bytes: Math.floor(rnd() * 80e9),
    family: i % 41 === 9,
    screenshots: i % 5 === 0,
    store: demoStore(i),
    proton: rnd() < 0.7 ? pick(['platinum', 'gold', 'silver', 'bronze', 'borked']) : null,
    cover: i % 23 === 5 ? null : demoCover(i),
    collections: [],
  };
}).filter((g, i, all) => COUNT > 400 || all.findIndex((x) => x.name === g.name) === i);


interface Col {
  id: string;
  name: string;
  apps: number[];
  dynamic: boolean;
}

const COLORS: Record<string, string> = { 'uc-completed': '#46a758', 'uc-started': '#0090ff', 'uc-unfinished': '#86b31a', 'uc-planned': '#8e4ec6' };

const initial: Col[] = [
  { id: 'uc-completed', name: 'Completed', apps: [], dynamic: false },
  { id: 'uc-started', name: 'Started', apps: [], dynamic: false },
  { id: 'uc-unfinished', name: 'Unfinished', apps: [], dynamic: false },
  { id: 'uc-planned', name: 'Planned', apps: [], dynamic: false },
  { id: 'uc-wontplay', name: "Probably won't play", apps: [], dynamic: false },
  { id: 'uc-ost', name: 'Soundtracks', apps: [], dynamic: true },
];
for (const g of games) {
  const done = g.achievements && g.achievements.unlocked === g.achievements.total;
  const target = done ? 'uc-completed' : g.playtime_minutes === 0 ? (rnd() < 0.7 ? 'uc-planned' : 'uc-wontplay') : now - g.last_played > 30 * DAY ? 'uc-unfinished' : 'uc-started';
  initial.find((c) => c.id === target)!.apps.push(g.appid);
}

let base: Col[] = structuredClone(initial);
let draft: Col[] = structuredClone(initial);
let undoStack: { label: string; cols: Col[] }[] = [];
let redoStack: { label: string; cols: Col[] }[] = [];
const snapshots: { id: string; label: string; taken_at: number; cols: Col[] }[] = [];
const smart: Record<string, SmartRule> = {};
let settings = {
  automation: null as string | null,
  online_stats: null as boolean | null,
  region: null as string | null,
  online_achievements: true,
  online_prices: true,
  online_protondb: true,
  online_hunters: true,
  reopen_steam: true,
  check_updates: true,
  update_channel: 'stable',
  account: null as number | null,
};
// Two invented accounts, so the switcher can be tried. The library doesn't change.
let mockAccount = 1;
let steamRunning = true;
let stopRequested = false;

function diff(from: Col[], to: Col[]): OpView[] {
  const names = new Map(games.map((g) => [g.appid, g.name]));
  const sample = (ids: number[]) => ids.slice(0, 8).map((a) => names.get(a) ?? '?');
  const ops: OpView[] = [];
  for (const t of to.filter((c) => !c.dynamic)) {
    const f = from.find((c) => c.id === t.id);
    if (!f) {
      ops.push({ kind: 'create', collection: t.name, count: t.apps.length, sample: sample(t.apps) });
      continue;
    }
    if (f.name !== t.name) ops.push({ kind: 'rename', collection: t.name, count: 0, sample: [] });
    const add = t.apps.filter((a) => !f.apps.includes(a));
    const rem = f.apps.filter((a) => !t.apps.includes(a));
    if (add.length) ops.push({ kind: 'add', collection: t.name, count: add.length, sample: sample(add) });
    if (rem.length) ops.push({ kind: 'remove', collection: t.name, count: rem.length, sample: sample(rem) });
  }
  for (const f of from.filter((c) => !c.dynamic && !to.some((t) => t.id === c.id))) ops.push({ kind: 'delete', collection: f.name, count: 0, sample: [] });
  return ops;
}

function changedIds(): string[] {
  return draft.filter((t) => {
    const f = base.find((c) => c.id === t.id);
    return !f || f.name !== t.name || f.apps.length !== t.apps.length || t.apps.some((a) => !f.apps.includes(a));
  }).map((c) => c.id);
}

const draftState = (): DraftState => ({
  pending: diff(base, draft).length,
  changed: changedIds(),
  undo: undoStack.at(-1)?.label ?? null,
  redo: redoStack.at(-1)?.label ?? null,
  steam_running: steamRunning,
  rebased: false,
});

const views = (): Collection[] => draft.map((c) => ({ id: c.id, name: c.name, count: c.apps.length, dynamic: c.dynamic, color: COLORS[c.id] ?? null }));

function membership(cols: Col[]) {
  const m = new Map<number, string[]>();
  for (const c of cols.filter((c) => !c.dynamic)) for (const a of c.apps) m.set(a, [...(m.get(a) ?? []), c.id]);
  return m;
}

function library(): Library {
  const m = membership(draft);
  return {
    games: games.map((g) => ({ ...g, collections: m.get(g.appid) ?? [] })),
    collections: views(),
    tags: TAGS,
    flags: FLAGS,
    draft: draftState(),
    ordered: false,
    online: { country: 'US', detected_region: 'US', profile_private: false, achievements_read: 42, usage: { month: '2026-11', month_requests: 951, month_bytes: 26e6, total_requests: 951, total_bytes: 26e6, last_run: now }, cached_regions: ['US'], sources: [{ key: 'online_achievements', label: 'Achievement progress', updated: now - 3600, items: 42 }, { key: 'online_prices', label: 'Prices and reviews', updated: now - 7200, items: 120 }, { key: 'online_hunters', label: 'Completion times', updated: now - 86400 * 3, items: 80 }, { key: 'online_protondb', label: 'ProtonDB ratings', updated: now - 86400 * 9, items: 90 }] },
  };
}

/** Runs an edit as one undo step and reports what changed, like the real backend. */
function edit(label: string, f: () => string | null | void): Patch {
  const before = structuredClone(draft);
  const beforeM = membership(draft);
  const value = f() ?? null;
  if (JSON.stringify(before) !== JSON.stringify(draft)) {
    undoStack.push({ label, cols: before });
    redoStack = [];
  }
  return patch(beforeM, value);
}

function patch(beforeM: Map<number, string[]>, value: string | null = null): Patch {
  const after = membership(draft);
  const changed: Record<string, string[]> = {};
  for (const id of new Set([...beforeM.keys(), ...after.keys()])) {
    if (JSON.stringify(beforeM.get(id) ?? []) !== JSON.stringify(after.get(id) ?? [])) changed[id] = after.get(id) ?? [];
  }
  return { collections: views(), draft: draftState(), membership: changed, value };
}

const find = (id: string) => {
  const c = draft.find((x) => x.id === id);
  if (!c) throw new Error(`collection ${id} not found`);
  return c;
};
const checkName = (name: string, except?: string) => {
  const n = name.trim();
  if (!n) throw new Error('give the collection a name');
  if (draft.some((c) => c.name.toLowerCase() === n.toLowerCase() && c.id !== except)) throw new Error(`a collection called '${n}' already exists`);
  return n;
};
const newId = () => `uc-${Math.random().toString(36).slice(2, 14)}`;
const withMembership = (g: Game) => ({ ...g, collections: membership(draft).get(g.appid) ?? [] });

const listeners = new Map<string, Set<(e: any) => void>>();
export function listen(event: string, handler: (e: any) => void) {
  if (!listeners.has(event)) listeners.set(event, new Set());
  listeners.get(event)!.add(handler);
  // The page can trigger events by hand for testing, e.g. __demoEmit('organizer', {...}).
  (window as any).__demoEmit = (name: string, e: any) => listeners.get(name)?.forEach((l) => l(e));
  (window as any).__demoSteam = (running: boolean) => {
    steamRunning = running;
    listeners.get('organizer')?.forEach((l) => l({ type: 'changed', steam_running: running }));
  };
  return Promise.resolve(() => listeners.get(event)?.delete(handler));
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

const sampleCrash = () => ({
  id: 'crash-1795760000.txt',
  at: Math.floor(Date.now() / 1000) - 60,
  summary: 'Depiled 0.2.5 stopped: index out of bounds',
  text: 'Depiled 0.2.5 stopped: index out of bounds: the len is 3 but the index is 7\nat src/service.rs:512 (thread main)\nWindows 11 Home 24H2 (build 26200), x86_64\nTime: 1795760000\n\nBacktrace:\n   0: <unknown>\n   1: <unknown>\n',
});

export async function call<T>(cmd: string, args: Record<string, any> = {}): Promise<T> {
  await sleep(60);
  const r = (x: unknown) => x as T;
  switch (cmd) {
    case 'status':
      return r({ steam_found: true, steam_running: steamRunning, legacy_debug_port: false });
    case 'disable_debug_port':
      return r(undefined);
    case 'get_settings':
      return r(structuredClone(settings));
    case 'set_settings':
      settings = structuredClone(args.settings);
      return r(undefined);
    case 'load_library':
      return r(library());
    case 'move_games':
      return r(edit(args.label, () => {
        for (const c of draft) {
          if (args.removeFrom.includes(c.id)) c.apps = c.apps.filter((a) => !args.appids.includes(a));
          if (args.addTo.includes(c.id)) c.apps.push(...args.appids.filter((a: number) => !c.apps.includes(a)));
        }
      }));
    case 'create_collection': {
      const name = checkName(args.name);
      return r(edit(`create ${name}`, () => {
        const id = newId();
        draft.push({ id, name, apps: [...args.appids], dynamic: false });
        return id;
      }));
    }
    case 'rename_collection': {
      const name = checkName(args.name, args.id);
      return r(edit(`rename ${find(args.id).name}`, () => void (find(args.id).name = name)));
    }
    case 'delete_collection':
      return r(edit(`delete ${find(args.id).name}`, () => void (draft = draft.filter((c) => c.id !== args.id))));
    case 'duplicate_collection': {
      const src = find(args.id);
      let name = `${src.name} copy`;
      for (let n = 2; draft.some((c) => c.name.toLowerCase() === name.toLowerCase()); n++) name = `${src.name} copy ${n}`;
      return r(edit(`create ${name}`, () => {
        const id = newId();
        draft.push({ id, name, apps: [...src.apps], dynamic: false });
        if (COLORS[src.id]) COLORS[id] = COLORS[src.id];
        return id;
      }));
    }
    case 'merge_collections': {
      const into = find(args.into);
      return r(edit(`merge into ${into.name}`, () => {
        for (const id of args.sources as string[]) {
          if (id === into.id) continue;
          into.apps.push(...find(id).apps.filter((a) => !into.apps.includes(a)));
          draft = draft.filter((c) => c.id !== id);
        }
      }));
    }
    case 'set_color':
      if (args.color) COLORS[args.id] = args.color;
      else delete COLORS[args.id];
      return r(patch(membership(draft)));
    case 'undo':
    case 'redo': {
      const [from, to] = cmd === 'undo' ? [undoStack, redoStack] : [redoStack, undoStack];
      const step = from.pop();
      const beforeM = membership(draft);
      if (step) {
        to.push({ label: step.label, cols: draft });
        draft = step.cols;
      }
      return r(patch(beforeM, step?.label ?? null));
    }
    case 'discard':
      return r(edit('discard changes', () => void (draft = structuredClone(base))));
    case 'pending':
      return r(diff(base, draft));
    case 'save': {
      if (steamRunning && !args.closeSteam) throw new Error('Close Steam first: it overwrites its collection files while it runs.');
      await sleep(900);
      const changes = diff(base, draft).length;
      const id = `${Date.now()}-save`;
      snapshots.unshift({ id, label: 'before saving', taken_at: Math.floor(Date.now() / 1000), cols: structuredClone(base) });
      base = structuredClone(draft);
      return r({ snapshot: id, changes, verified: true });
    }
    case 'smart_rules':
      return r(structuredClone(smart));
    case 'create_smart_collection': {
      const name = checkName(args.name);
      const hits = games.filter((g) => g.kind === 'game' && matchesRule(args.rule, withMembership(g))).map((g) => g.appid);
      return r(edit(`create ${name}`, () => {
        const id = newId();
        draft.push({ id, name, apps: hits, dynamic: false });
        if (args.auto) smart[id] = args.rule;
        return `${id}:${hits.length}`;
      }));
    }
    case 'set_smart_rule':
      if (args.rule) smart[args.id] = args.rule;
      else delete smart[args.id];
      return r(undefined);
    case 'fill_collection': {
      const c = find(args.id);
      const rule = smart[args.id];
      if (!rule) throw new Error(`'${c.name}' has no auto-fill criteria yet`);
      const hits = games.filter((g) => g.kind === 'game' && !c.apps.includes(g.appid) && matchesRule(rule, withMembership(g))).map((g) => g.appid);
      return r(edit(`fill ${c.name}`, () => {
        c.apps.push(...hits);
        return String(hits.length);
      }));
    }
    case 'preview_smart': {
      const hits = games.filter((g) => g.kind === 'game' && matchesRule(args.rule, withMembership(g)));
      const c = args.collection ? find(args.collection) : null;
      return r([hits.length, hits.filter((g) => !c?.apps.includes(g.appid)).length]);
    }
    case 'history':
      return r(snapshots.map(({ id, label, taken_at, cols }): SnapshotInfo => ({ id, label, taken_at, collections: cols.length })));
    case 'restore_snapshot': {
      const snap = snapshots.find((s) => s.id === args.id);
      if (!snap) throw new Error('snapshot not found');
      return r(edit('restore an earlier version', () => void (draft = structuredClone(snap.cols))));
    }
    case 'preview_rules': {
      const idle = games.filter((g) => g.playtime_minutes > 0 && now - g.last_played > 7 * DAY && !find('uc-unfinished').apps.includes(g.appid));
      return r({ ops: idle.length ? [{ kind: 'add', collection: 'Unfinished', count: idle.length, sample: idle.slice(0, 8).map((g) => g.name) }] : [], notes: [] });
    }
    case 'apply_rules':
      return r(edit('auto-organize', () => {
        const idle = games.filter((g) => g.playtime_minutes > 0 && now - g.last_played > 7 * DAY).map((g) => g.appid);
        const started = find('uc-started');
        started.apps = started.apps.filter((a) => !idle.includes(a));
        const u = find('uc-unfinished');
        u.apps.push(...idle.filter((a) => !u.apps.includes(a)));
      }));
    case 'refresh_online': {
      // Pretend to fetch, so the progress panel can be seen in the demo.
      const emit = (e: unknown) => listeners.get('online')?.forEach((l) => l(e));
      let done = 0;
      const t = setInterval(() => {
        done += 7;
        if (done >= 60 || stopRequested) {
          clearInterval(t);
          stopRequested = false;
          emit({ task: '', done: 0, total: 0, finished: true, errors: [] });
        } else emit({ task: 'achievement progress', done, total: 60, finished: false, errors: [] });
      }, 500);
      // "Fetch everything now" pretends to be a big library's first fetch, to show the notice.
      return r(args.force ? { requests: 6400, seconds: 2400, games: 5120 } : { requests: 12, seconds: 20, games: 40 });
    }
    case 'stop_online':
      stopRequested = true;
      return r(undefined);
    case 'set_ordering':
      return r(edit('order', () => null));
    case 'set_order': {
      const ids: string[] = args.ids;
      return r(edit('reorder collections', () => {
        const at = (c: Col) => (ids.includes(c.id) ? ids.indexOf(c.id) : ids.length);
        draft.sort((a, b) => at(a) - at(b));
        return null;
      }));
    }
    case 'presets': {
      const t = (field: string, op: string, value: unknown) => ({ field, op, value });
      const rule = (...all: unknown[]) => ({ played: null, achievements: null, min_review: 0, flags: [], tags: [], condition: { all }, sync: true });
      return r([
        { id: 'achievements', name: 'Achievement hunting', description: 'Games with achievements, by how far you’ve got.', collections: [
          { name: 'Completed', rule: rule(t('completed', 'is', true)) },
          { name: 'In progress', rule: rule(t('has_achievements', 'is', true), t('achievements_unlocked', 'gt', 0), t('completed', 'is', false)) },
          { name: 'Haven’t started', rule: rule(t('has_achievements', 'is', true), t('achievements_unlocked', 'eq', 0)) },
        ] },
        { id: 'deck', name: 'Steam Deck', description: 'Valve’s Steam Deck rating for each game.', collections: [
          { name: 'Deck Verified', rule: rule(t('deck', 'is', 'verified')) },
          { name: 'Deck Playable', rule: rule(t('deck', 'is', 'playable')) },
        ] },
      ]);
    }
    case 'apply_preset': {
      const made = ['In progress', 'Haven’t started'].filter((n) => !draft.some((c) => c.name === n));
      const p = edit('preset', () => { for (const name of made) draft.push({ id: newId(), name, apps: [], dynamic: false }); return null; });
      return r({ patch: p, created: made, attached: [], kept: ['Completed'] });
    }
    case 'crash_mode':
      // ?crash in the demo's address shows the crash window.
      return r(new URLSearchParams(location.search).has('crash') ? sampleCrash() : null);
    case 'pending_crashes':
      return r(new URLSearchParams(location.search).has('crashed') ? [sampleCrash()] : []);
    case 'settle_crash':
      return r(undefined);
    case 'can_send_reports':
      return r(true);
    case 'report_details':
      return r(`Depiled 0.2.5\nWindows 11 Home 24H2 (build 26200), x86_64\nLibrary: ${games.length} entries, 6 collections\nSteam accounts on this computer: 2\nOrganize as I play: notify\nDetails from the internet: on\nUpdates: stable\nSteam running: true${args.recentErrors?.length ? '\n\nRecent errors, newest last:\n' + args.recentErrors.map((e: string) => '- ' + e).join('\n') : ''}`);
    case 'send_report':
      await sleep(700);
      return r(undefined);
    case 'report_text':
      return r(`Depiled ${args.report.kind} report\n\n${args.report.message}`);
    case 'start_normally':
      location.search = '';
      return r(undefined);
    case 'accounts':
      return r({
        accounts: [
          { id: 1, name: 'Sam', login: 'sam_plays', avatar: null, pending: 0 },
          { id: 2, name: 'Sam (streaming)', login: 'sam_live', avatar: null, pending: 3 },
        ],
        current: mockAccount,
        picked: settings.account,
        signed_in: 1,
      });
    case 'set_account':
      settings.account = args.account;
      mockAccount = args.account ?? 1;
      return r(undefined);
    case 'check_update':
      return r({ version: '0.3.0', current: '0.2.5', notes: null });
    case 'install_update': {
      // Pretend to download, so the progress can be seen in the demo. Nothing restarts.
      const total = 6_400_000;
      for (let downloaded = 0; downloaded <= total; downloaded += 320_000) {
        listeners.get('update-progress')?.forEach((l) => l({ downloaded, total }));
        await sleep(120);
      }
      throw new Error('The demo can’t install updates.');
    }
  }
  throw new Error(`unknown command ${cmd}`);
}
