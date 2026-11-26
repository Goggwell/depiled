import type { Game, SmartRule } from './api';

export const IDLE_DAYS = 7;
const DAY = 86400;

// ---- conditions with AND, OR and NOT -----------------------------------------------------
// Same shape and meaning as src/cond.rs, which applies them when rules run. Unknown values
// (no price, never played) never match a comparison; 'known' and 'unknown' test for them.

export type Cond = { all: Cond[] } | { any: Cond[] } | { not: Cond } | Test;
export interface Test {
  field: FieldId;
  op: string;
  value?: unknown;
}

export type FieldId =
  | 'name' | 'developer' | 'publisher'
  | 'playtime_hours' | 'last_played_days' | 'release_year' | 'review_percent' | 'review_count' | 'metacritic'
  | 'achievements' | 'achievements_unlocked' | 'achievement_percent' | 'perfect_hours' | 'price' | 'discount' | 'size_gb'
  | 'played' | 'deck' | 'proton'
  | 'installed' | 'vr' | 'early_access' | 'family' | 'on_sale' | 'completed'
  | 'tags' | 'features' | 'platforms' | 'collections';

export type FieldType = 'text' | 'num' | 'choice' | 'bool' | 'set';

export interface Field {
  id: FieldId;
  label: string;
  type: FieldType;
  group: string;
  /** For 'choice' fields, and fixed 'set' fields. */
  options?: [string, string][];
  /** Shown after a number, e.g. h or %. */
  unit?: string;
}

export const FIELDS: Field[] = [
  { id: 'name', label: 'Name', type: 'text', group: 'Game' },
  { id: 'developer', label: 'Developer', type: 'text', group: 'Game' },
  { id: 'publisher', label: 'Publisher', type: 'text', group: 'Game' },
  { id: 'tags', label: 'Genres and tags', type: 'set', group: 'Game' },
  { id: 'features', label: 'Features', type: 'set', group: 'Game' },
  { id: 'release_year', label: 'Release year', type: 'num', group: 'Game' },
  { id: 'played', label: 'Play status', type: 'choice', group: 'Playing', options: [['never', 'Never played'], ['played', 'Played']] },
  { id: 'playtime_hours', label: 'Time played', type: 'num', unit: 'h', group: 'Playing' },
  { id: 'last_played_days', label: 'Last played', type: 'num', unit: 'ago', group: 'Playing' },
  { id: 'achievements', label: 'Number of achievements', type: 'num', group: 'Achievements' },
  { id: 'achievements_unlocked', label: 'Achievements unlocked', type: 'num', group: 'Achievements' },
  { id: 'achievement_percent', label: 'Achievements unlocked (%)', type: 'num', unit: '%', group: 'Achievements' },
  { id: 'completed', label: 'Every achievement unlocked', type: 'bool', group: 'Achievements' },
  { id: 'perfect_hours', label: 'Time to 100% (median)', type: 'num', unit: 'h', group: 'Achievements' },
  { id: 'review_percent', label: 'Positive reviews', type: 'num', unit: '%', group: 'Store' },
  { id: 'review_count', label: 'Number of reviews', type: 'num', group: 'Store' },
  { id: 'metacritic', label: 'Metacritic score', type: 'num', group: 'Store' },
  { id: 'price', label: 'Price', type: 'num', group: 'Store' },
  { id: 'discount', label: 'Discount', type: 'num', unit: '%', group: 'Store' },
  { id: 'on_sale', label: 'On sale', type: 'bool', group: 'Store' },
  { id: 'installed', label: 'Installed', type: 'bool', group: 'This computer' },
  { id: 'size_gb', label: 'Size on disk', type: 'num', unit: 'GB', group: 'This computer' },
  { id: 'family', label: 'Family game', type: 'bool', group: 'This computer' },
  { id: 'collections', label: 'Collections', type: 'set', group: 'This computer' },
  { id: 'deck', label: 'Steam Deck', type: 'choice', group: 'Platforms', options: [['verified', 'Verified'], ['playable', 'Playable'], ['unsupported', 'Unsupported'], ['unknown', 'Unknown']] },
  { id: 'platforms', label: 'Runs on', type: 'set', group: 'Platforms', options: [['windows', 'Windows'], ['mac', 'macOS'], ['linux', 'Linux']] },
  {
    id: 'proton',
    label: 'Linux rating (ProtonDB)',
    type: 'choice',
    group: 'Platforms',
    options: [['platinum', 'Platinum'], ['gold', 'Gold'], ['silver', 'Silver'], ['bronze', 'Bronze'], ['borked', 'Doesn’t run'], ['pending', 'Too few reports'], ['none', 'No reports']],
  },
  { id: 'vr', label: 'VR', type: 'bool', group: 'Platforms' },
  { id: 'early_access', label: 'Early access', type: 'bool', group: 'Platforms' },
];

export const fieldById = (id: string) => FIELDS.find((f) => f.id === id);

export const OPS: Record<FieldType, [string, string][]> = {
  text: [['contains', 'contains'], ['not_contains', 'doesn’t contain'], ['is', 'is'], ['is_not', 'isn’t'], ['starts_with', 'starts with']],
  num: [['lt', 'less than'], ['lte', 'at most'], ['gt', 'more than'], ['gte', 'at least'], ['eq', 'exactly'], ['between', 'between'], ['known', 'is known'], ['unknown', 'is unknown']],
  choice: [['is', 'is'], ['is_not', 'isn’t'], ['in', 'is one of']],
  bool: [['is', 'is']],
  set: [['has_any', 'has any of'], ['has_all', 'has all of'], ['has_none', 'has none of']],
};

export const isGroup = (c: Cond): c is { all: Cond[] } | { any: Cond[] } => 'all' in c || 'any' in c;
export const groupItems = (c: Cond): Cond[] => ('all' in c ? c.all : 'any' in c ? c.any : []);

/** Number of single tests in a condition. */
export function countTests(c: Cond): number {
  if ('all' in c) return c.all.reduce((n, x) => n + countTests(x), 0);
  if ('any' in c) return c.any.reduce((n, x) => n + countTests(x), 0);
  if ('not' in c) return countTests(c.not);
  return 1;
}

/** Ignore unfinished rows while the user builds a rule. Empty subgroups must not
 * turn an OR into a match for the entire library. */
export function readyCondition(c: Cond): Cond | null {
  if ('not' in c) {
    const child = readyCondition(c.not);
    return child ? { not: child } : null;
  }
  if (isGroup(c)) {
    const children = groupItems(c).map(readyCondition).filter((x): x is Cond => x !== null);
    return children.length ? ('all' in c ? { all: children } : { any: children }) : null;
  }
  if (c.op === 'known' || c.op === 'unknown') return c;
  const v = c.value;
  if (c.op === 'between') {
    if (!Array.isArray(v)) return null;
    const [lo, hi] = v.map(num);
    return (lo != null || hi != null) && (lo == null || hi == null || lo <= hi) ? c : null;
  }
  if (Array.isArray(v)) return v.length ? c : null;
  return v == null || (typeof v === 'string' && !v.trim()) ? null : c;
}

/** The editor expresses negation as "none of these", including imported NOT leaves. */
export function editableGroup(c: Cond): Cond {
  function visit(node: Cond): Cond {
    if ('not' in node) {
      const child = visit(node.not);
      return { not: 'any' in child ? child : { any: [child] } };
    }
    if ('all' in node) return { all: node.all.map(visit) };
    if ('any' in node) return { any: node.any.map(visit) };
    return { ...node };
  }
  const root = visit(c);
  return 'field' in root ? { all: [root] } : root;
}

const year = (ts: number) => new Date(ts * 1000).getUTCFullYear();

// ---- matching ------------------------------------------------------------------------------
// A condition is compiled once into a function of the game: field lookups, lower-casing, number
// parsing and value sets are all worked out up front, so filtering a library of thousands runs
// only the comparisons. Same results as src/cond.rs.

type Pred = (g: Game, now: number) => boolean;

const num = (v: unknown): number | null => (typeof v === 'number' ? v : typeof v === 'string' && v.trim() !== '' && !isNaN(Number(v)) ? Number(v) : null);
/** A value as it is compared: numbers (also written as text) as numbers, other text without case. */
const key = (v: unknown): string | number | null => num(v) ?? (typeof v === 'string' ? v.toLowerCase() : null);

const pos = (v: number) => (v > 0 ? v : null);
const total = (g: Game) => Math.max(g.achievements?.total ?? 0, g.hunters?.achievements ?? 0);
/** Progress Steam or the profile reported, for games that have achievements. */
const progress = (g: Game) => (g.achievements && g.achievements.total > 0 ? g.achievements : null);
const DECK = ['unknown', 'unsupported', 'playable', 'verified'];
const PLATFORMS: [string, number][] = [['windows', 1], ['mac', 2], ['linux', 4]];

/** Each field's value for a game. Unknown numbers are null. */
const READ: { [K in FieldId]: (g: Game, now: number) => unknown } = {
  name: (g) => [g.name],
  developer: (g) => g.developers,
  publisher: (g) => g.publishers,
  playtime_hours: (g) => g.playtime_minutes / 60,
  last_played_days: (g, now) => (g.last_played > 0 ? Math.floor(Math.max(0, now - g.last_played) / DAY) : null),
  release_year: (g) => (g.release > 0 ? year(g.release) : null),
  review_percent: (g) => pos(g.review_percent),
  review_count: (g) => pos(g.store?.review_count ?? 0),
  metacritic: (g) => pos(g.metacritic),
  achievements: (g) => pos(total(g)),
  achievements_unlocked: (g) => progress(g)?.unlocked ?? null,
  achievement_percent: (g) => {
    const a = progress(g);
    return a ? (a.unlocked * 100) / Math.max(a.total, total(g)) : null;
  },
  perfect_hours: (g) => pos((g.hunters?.median_minutes ?? 0) / 60),
  price: (g) => (g.store?.price_cents != null ? g.store.price_cents / 100 : null),
  discount: (g) => g.store?.discount ?? 0,
  size_gb: (g) => pos(g.installed_bytes / 1073741824),
  played: (g) => (g.playtime_minutes > 0 || g.last_played > 0 ? 'played' : 'never'),
  deck: (g) => DECK[Math.min(3, g.extras.deck)],
  proton: (g) => g.proton || 'none',
  installed: (g) => g.installed_bytes > 0,
  vr: (g) => g.extras.vr,
  early_access: (g) => g.extras.early_access,
  family: (g) => g.family,
  on_sale: (g) => (g.store?.discount ?? 0) > 0,
  completed: (g) => {
    const a = progress(g);
    return !!a && a.unlocked >= Math.max(a.total, total(g));
  },
  tags: (g) => g.tags,
  features: (g) => g.categories,
  platforms: (g) => PLATFORMS.filter(([, b]) => g.extras.platforms & b).map(([n]) => n),
  collections: (g) => g.collections,
};

const never: Pred = () => false;
const not = (p: Pred): Pred => (g, now) => !p(g, now);

function compileTest(t: Test): Pred {
  const field = fieldById(t.field);
  if (!field) return never;
  const read = READ[field.id];
  const value = t.value;
  switch (field.type) {
    case 'text': {
      const want = String(value ?? '').trim().toLowerCase();
      const some = (p: (s: string) => boolean): Pred => (g, now) => (read(g, now) as string[]).some((s) => p(s.toLowerCase()));
      if (t.op === 'contains') return some((s) => s.includes(want));
      if (t.op === 'not_contains') return not(some((s) => s.includes(want)));
      if (t.op === 'is') return some((s) => s === want);
      if (t.op === 'is_not') return not(some((s) => s === want));
      if (t.op === 'starts_with') return some((s) => s.startsWith(want));
      return never;
    }
    case 'num': {
      if (t.op === 'known') return (g, now) => read(g, now) !== null;
      if (t.op === 'unknown') return (g, now) => read(g, now) === null;
      let ok: (x: number) => boolean;
      if (t.op === 'between') {
        // A range needs both ends written down, even if one is left open (null).
        if (!Array.isArray(value)) return never;
        const lo = num(value[0]);
        const hi = num(value[1]);
        ok = (x) => (lo === null || x >= lo) && (hi === null || x <= hi);
      } else {
        const v = num(value);
        if (v === null) return never;
        const ops: Record<string, (x: number) => boolean> = { lt: (x) => x < v, lte: (x) => x <= v, gt: (x) => x > v, gte: (x) => x >= v, eq: (x) => Math.abs(x - v) < 1e-9 };
        ok = ops[t.op] ?? never;
      }
      return (g, now) => {
        const x = read(g, now) as number | null;
        return x !== null && ok(x);
      };
    }
    case 'choice': {
      const one = key(value);
      const many = new Set(Array.isArray(value) ? value.map(key) : []);
      const is: Pred = (g, now) => one !== null && key(read(g, now)) === one;
      if (t.op === 'is') return is;
      if (t.op === 'is_not') return not(is);
      if (t.op === 'in') return Array.isArray(value) ? (g, now) => many.has(key(read(g, now))) : never;
      return never;
    }
    case 'bool': {
      // A missing value means "yes".
      const want = typeof value === 'boolean' ? value : true;
      return t.op === 'is' ? (g, now) => read(g, now) === want : never;
    }
    case 'set': {
      const wanted = (Array.isArray(value) ? value : [value]).map(key);
      const lookup = new Set<string | number | null>(wanted.filter((w) => w !== null));
      const any: Pred = (g, now) => (read(g, now) as unknown[]).some((h) => lookup.has(key(h)));
      if (t.op === 'has_any') return any;
      if (t.op === 'has_none') return not(any);
      if (t.op === 'has_all')
        return (g, now) => {
          const have = new Set((read(g, now) as unknown[]).map(key));
          return wanted.every((w) => w !== null && have.has(w));
        };
      return never;
    }
  }
}

/** Compiles a condition into a function of the game. Compile once, call for every game. */
export function compileCond(c: Cond): Pred {
  if ('all' in c) {
    const parts = c.all.map(compileCond);
    return (g, now) => parts.every((p) => p(g, now));
  }
  if ('any' in c) {
    // An empty group matches everything, like an empty filter.
    if (c.any.length === 0) return () => true;
    const parts = c.any.map(compileCond);
    return (g, now) => parts.some((p) => p(g, now));
  }
  if ('not' in c) return not(compileCond(c.not));
  return compileTest(c);
}

export function matchesCond(c: Cond, g: Game, now = Date.now() / 1000): boolean {
  return compileCond(c)(g, now);
}

// ---- simple criteria (the quick filters) -------------------------------------------------

export const emptyRule = (): SmartRule => ({ played: null, achievements: null, min_review: 0, flags: [], tags: [], condition: null });

export const ruleIsEmpty = (r: SmartRule) =>
  !r.played && !r.achievements && r.min_review === 0 && r.flags.length === 0 && r.tags.length === 0 && (!r.condition || countTests(r.condition) === 0);

/** Same meaning as the Rust side: every condition that is set must hold. */
export function compileRule(rule: SmartRule): Pred {
  const cond = rule.condition ? compileCond(rule.condition) : null;
  const flags = [...rule.flags];
  const tags = [...rule.tags];
  const { played, achievements, min_review } = rule;
  return (g, now) => {
    const started = g.playtime_minutes > 0 || g.last_played > 0;
    if (played === 'never' && started) return false;
    if (played === 'played' && !started) return false;
    if (played === 'idle' && !(started && g.last_played > 0 && now - g.last_played >= IDLE_DAYS * DAY)) return false;
    if (achievements) {
      const a = g.achievements;
      // Unknown progress, or a game with no achievements, is neither complete nor in progress.
      if (!a || a.total === 0 || (achievements === 'complete') !== (a.unlocked === a.total)) return false;
    }
    if (g.review_percent < min_review) return false;
    if (cond && !cond(g, now)) return false;
    return flags.every((f) => g.categories.includes(f)) && tags.every((t) => g.tags.includes(t));
  };
}

// Rules are compiled on first use. Every change to the filters makes a new rule object, so a
// cached function never outlives the rule it was made from.
const compiled = new WeakMap<SmartRule, Pred>();

export function matchesRule(rule: SmartRule, g: Game, now = Date.now() / 1000): boolean {
  let p = compiled.get(rule);
  if (!p) compiled.set(rule, (p = compileRule(rule)));
  return p(g, now);
}
