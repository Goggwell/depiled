// Demo backend used when the page runs in a normal browser, so the design can be worked on
// without Steam. It holds invented games in memory and behaves like the real commands.
import type { Achievements, Collection, Game, Library, OpView, SmartRule, SnapshotInfo } from './api';
import { matchesRule } from './criteria';

const WORDS_A = ['Neon', 'Hollow', 'Crimson', 'Quiet', 'Iron', 'Lunar', 'Rusted', 'Velvet', 'Ember', 'Glass', 'Wild', 'Paper', 'Distant', 'Silent', 'Golden'];
const WORDS_B = ['Knight', 'Garden', 'Signal', 'Frontier', 'Harbor', 'Circuit', 'Tide', 'Citadel', 'Orchard', 'Descent', 'Odyssey', 'Protocol', 'Kingdom', 'Reverie', 'Outpost'];
const TAGS: Record<string, string> = {
  '19': 'Action', '21': 'Adventure', '122': 'RPG', '9': 'Strategy', '597': 'Casual', '1667': 'Horror', '1716': 'Roguelike',
  '1742': 'Story Rich', '3942': 'Cozy', '1664': 'Puzzle', '1685': 'Co-op', '4182': 'Singleplayer', '5395': '3D Platformer', '1695': 'Open World',
};
const FLAGS = [
  { id: 2, name: 'single-player' }, { id: 1, name: 'multi-player' }, { id: 9, name: 'co-op' },
  { id: 23, name: 'steam cloud' }, { id: 28, name: 'full controller support' }, { id: 22, name: 'steam achievements' },
];

let seed = 7;
const rnd = () => ((seed = (seed * 16807) % 2147483647) / 2147483647);
const pick = <T,>(xs: T[]) => xs[Math.floor(rnd() * xs.length)];

const now = Math.floor(Date.now() / 1000);
const DAY = 86400;

/** `?games=10000` in the page address makes a bigger invented library, to test performance. */
const COUNT = Number(new URLSearchParams(globalThis.location?.search ?? '').get('games')) || 140;

const games: Game[] = Array.from({ length: COUNT }, (_, i) => {
  const never = rnd() < 0.35;
  const minutes = never ? 0 : Math.floor(rnd() ** 2 * 6000) + 15;
  const tagIds = Object.keys(TAGS).filter(() => rnd() < 0.22).map(Number);
  return {
    appid: 1000 + i,
    name: `${pick(WORDS_A)} ${pick(WORDS_B)}${rnd() < 0.2 ? ' II' : ''}${COUNT > 400 ? ` ${i}` : ''}`,
    playtime_minutes: minutes,
    last_played: never ? 0 : now - Math.floor(rnd() * 120) * DAY,
    categories: FLAGS.filter(() => rnd() < 0.5).map((f) => f.id),
    tags: tagIds,
    metacritic: rnd() < 0.5 ? Math.floor(60 + rnd() * 38) : 0,
    review_percent: Math.floor(55 + rnd() * 45),
    cover: null,
    collections: [],
  };
}).filter((g, i, all) => COUNT > 400 || all.findIndex((x) => x.name === g.name) === i);

let collections: Collection[] = [
  { id: 'uc-completed', name: 'COMPLETED', count: 0, dynamic: false },
  { id: 'uc-started', name: 'STARTED', count: 0, dynamic: false },
  { id: 'uc-unfinished', name: 'UNFINISHED', count: 0, dynamic: false },
  { id: 'uc-planned', name: 'PLANNED', count: 0, dynamic: false },
  { id: 'uc-wontplay', name: "PROBABLY WON'T PLAY", count: 0, dynamic: false },
  { id: 'uc-ost', name: 'Soundtracks', count: 0, dynamic: true },
];

for (const g of games) {
  if (g.playtime_minutes === 0) g.collections.push(rnd() < 0.7 ? 'uc-planned' : 'uc-wontplay');
  else if (now - g.last_played > 30 * DAY) g.collections.push('uc-unfinished');
  else g.collections.push('uc-started');
}

const recount = () => {
  for (const c of collections) c.count = games.filter((g) => g.collections.includes(c.id)).length;
};
recount();

const stack: { id: string; label: string; taken_at: number; state: string }[] = [];
const checkpoint = (label: string) => {
  const id = `${Date.now()}-${label.replace(/\W+/g, '-')}`;
  stack.unshift({ id, label, taken_at: Math.floor(Date.now() / 1000), state: JSON.stringify({ games, collections }) });
  return id;
};

const smart: Record<string, SmartRule> = {};

function mockAchievements(): Record<string, Achievements> {
  const out: Record<string, Achievements> = {};
  for (const g of games) if (g.playtime_minutes > 0) {
    const total = 10 + (g.appid % 40);
    out[g.appid] = { total, unlocked: g.appid % 5 === 0 ? total : Math.floor(total * ((g.appid % 9) / 10)) };
  }
  return out;
}

let settings: { automation: 'off' | 'notify' | 'auto' | null } = { automation: null };
const listeners = new Set<(e: any) => void>();

/** Demo mode has no Steam to watch, so the page can trigger the event by hand for testing. */
export function onOrganizer(handler: (e: any) => void) {
  listeners.add(handler);
  (window as any).__demoOrganizer = (e: any) => listeners.forEach((l) => l(e));
  return () => listeners.delete(handler);
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

export async function call<T>(cmd: string, args: Record<string, any> = {}): Promise<T> {
  await sleep(120);
  switch (cmd) {
    case 'status':
      return { steam_found: true, steam_running: true, ready: true, flag_file: true } as T;
    case 'enable_setup':
      return undefined as T;
    case 'get_settings':
      return structuredClone(settings) as T;
    case 'set_settings':
      settings = structuredClone(args.settings);
      return undefined as T;
    case 'load_library':
      recount();
      return structuredClone({ games, collections, tags: TAGS, flags: FLAGS }) as T;
    case 'load_achievements':
      return mockAchievements() as T;
    case 'smart_rules':
      return structuredClone(smart) as T;
    case 'create_smart_collection': {
      if (collections.some((c) => c.name.toLowerCase() === args.name.trim().toLowerCase())) throw new Error(`a collection called '${args.name}' already exists`);
      const id = checkpoint(`create ${args.name}`);
      const cid = `uc-${Math.random().toString(36).slice(2, 8)}`;
      collections.push({ id: cid, name: args.name, count: 0, dynamic: false });
      const hits = games.filter((g) => matchesRule(args.rule, g, mockAchievements()));
      for (const g of hits) g.collections.push(cid);
      if (args.auto) smart[cid] = args.rule;
      return { snapshot: id, added: hits.length } as T;
    }
    case 'set_smart_rule':
      if (args.rule) smart[args.id] = args.rule;
      else delete smart[args.id];
      return undefined as T;
    case 'fill_collection': {
      const rule = smart[args.id];
      if (!rule) throw new Error('this collection has no auto-fill criteria yet');
      const hits = games.filter((g) => !g.collections.includes(args.id) && matchesRule(rule, g, mockAchievements()));
      const id = hits.length ? checkpoint('fill collection') : null;
      for (const g of hits) g.collections.push(args.id);
      return { snapshot: id, added: hits.length } as T;
    }
    case 'move_games': {
      const id = checkpoint(args.label);
      for (const g of games.filter((x) => args.appids.includes(x.appid))) {
        g.collections = g.collections.filter((c) => !args.removeFrom.includes(c));
        for (const c of args.addTo) if (!g.collections.includes(c)) g.collections.push(c);
      }
      return id as T;
    }
    case 'create_collection': {
      const id = checkpoint(`create ${args.name}`);
      const cid = `uc-${Math.random().toString(36).slice(2, 8)}`;
      collections.push({ id: cid, name: args.name, count: 0, dynamic: false });
      for (const g of games.filter((x) => args.appids.includes(x.appid))) g.collections.push(cid);
      return id as T;
    }
    case 'rename_collection': {
      const id = checkpoint(`rename ${args.name}`);
      collections.find((c) => c.id === args.id)!.name = args.name;
      return id as T;
    }
    case 'delete_collection': {
      const id = checkpoint('delete collection');
      collections = collections.filter((c) => c.id !== args.id);
      for (const g of games) g.collections = g.collections.filter((c) => c !== args.id);
      return id as T;
    }
    case 'history':
      return stack.map(({ id, label, taken_at }): SnapshotInfo => ({ id, label, taken_at, collections: collections.length })) as T;
    case 'restore_snapshot': {
      const snap = stack.find((s) => s.id === args.id);
      if (!snap) throw new Error('snapshot not found');
      const state = JSON.parse(snap.state);
      games.splice(0, games.length, ...state.games);
      collections = state.collections; // exact restore: collections made since vanish
      return { changes: ['restored'], safety_snapshot: null, verified: true } as T;
    }
    case 'preview_rules': {
      const idle = games.filter((g) => g.playtime_minutes > 0 && now - g.last_played > 7 * DAY && !g.collections.includes('uc-unfinished'));
      const ops: OpView[] = idle.length
        ? [{ kind: 'add', collection: 'UNFINISHED', count: idle.length, sample: idle.slice(0, 6).map((g) => g.name) }]
        : [];
      return { ops, notes: [] } as T;
    }
    case 'apply_rules': {
      const id = checkpoint('auto-organize');
      for (const g of games.filter((x) => x.playtime_minutes > 0 && now - x.last_played > 7 * DAY)) {
        g.collections = g.collections.filter((c) => c !== 'uc-started');
        if (!g.collections.includes('uc-unfinished')) g.collections.push('uc-unfinished');
      }
      return { snapshot: id, applied: 1, verified: true } as T;
    }
  }
  throw new Error(`unknown command ${cmd}`);
}

export type { Library };
