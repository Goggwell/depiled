import * as api from './api';
import type { Achievements, Automation, Game, Library, OpView, OrganizerEvent, Settings, SmartRule, Status } from './api';
import { emptyRule, matchesRule } from './criteria';

export type PlayedFilter = 'any' | 'never' | 'played' | 'idle';
export type AchFilter = 'any' | 'complete' | 'incomplete';
export type Sort = 'name' | 'playtime' | 'recent' | 'review';
/** 'new' creates a collection; 'rule' edits the auto-fill criteria of `ruleFor`. */
export type Dialog = null | 'organize' | 'history' | 'new' | 'rule' | 'settings';
export type Theme = 'system' | 'light' | 'dark';

const THEME_KEY = 'depiled-theme';
const COLLATOR = new Intl.Collator();
const THEME_NAMES = { light: 'olive-day', dark: 'olive-night' } as const;

const DAY = 86400;

interface Toast {
  text: string;
  canUndo: boolean;
  error?: boolean;
}

class AppStore {
  status = $state<Status | null>(null);
  // `$state.raw` for the big, replace-as-a-whole data: it is never edited in place, and not
  // wrapping tens of thousands of games in reactive proxies saves a lot of memory and time.
  library = $state.raw<Library | null>(null);
  achievements = $state.raw<Record<string, Achievements>>({});
  loading = $state(false);
  busy = $state(false);

  search = $state('');
  /** 'all', 'none' (in no collection), or a collection id. */
  view = $state('all');
  tagFilter = $state<number[]>([]);
  flagFilter = $state<number[]>([]);
  played = $state<PlayedFilter>('any');
  ach = $state<AchFilter>('any');
  minReview = $state(0);
  sort = $state<Sort>('name');
  layout = $state<'grid' | 'list'>('grid');

  selected = $state.raw<number[]>([]);
  anchor: number | null = null;

  settings = $state<Settings>({ automation: null });
  settingsLoaded = $state(false);
  promptDismissed = $state(false);
  /** Changes the watcher found and is waiting for the user to approve. */
  pending = $state<OpView[] | null>(null);
  theme = $state<Theme>('system');

  /** Asked once, the first time, how automatic things should be. */
  showAutomationPrompt = $derived(this.settingsLoaded && this.settings.automation === null && !this.promptDismissed);

  dialog = $state<Dialog>(null);
  toast = $state<Toast | null>(null);
  undoStack = $state<{ id: string; label: string }[]>([]);
  private toastTimer: ReturnType<typeof setTimeout> | undefined;

  collections = $derived(this.library?.collections ?? []);
  tagNames = $derived(this.library?.tags ?? {});

  /** Tags worth offering as filters: common ones first. */
  tagChoices = $derived.by(() => {
    const counts = new Map<number, number>();
    for (const g of this.library?.games ?? []) for (const t of g.tags) counts.set(t, (counts.get(t) ?? 0) + 1);
    return [...counts.entries()]
      .filter(([, n]) => n >= 3)
      .sort((a, b) => b[1] - a[1])
      .map(([id, n]) => ({ id, name: this.tagNames[id] ?? String(id), count: n }));
  });

  hasAchievements = $derived(Object.keys(this.achievements).length > 0);

  /** The current filters as a rule, ignoring the search box and which collection is open. */
  criteria = $derived<SmartRule>({
    played: this.played === 'any' ? null : this.played,
    achievements: this.ach === 'any' ? null : this.ach,
    min_review: this.minReview,
    flags: this.flagFilter,
    tags: this.tagFilter,
  });

  /** Every game the current filters match, wherever it is filed. */
  criteriaMatches = $derived((this.library?.games ?? []).filter((g) => matchesRule(this.criteria, g, this.achievements)));

  /** What each collection auto-fills with, by collection id. */
  smartRules = $state.raw<Record<string, SmartRule>>({});
  /** Collection whose criteria the 'rule' dialog edits. */
  ruleFor = $state<string | null>(null);

  /** Games that fit a collection's saved criteria but aren't in it yet. */
  newMatches(collectionId: string): Game[] {
    const rule = this.smartRules[collectionId];
    if (!rule) return [];
    return (this.library?.games ?? []).filter((g) => !g.collections.includes(collectionId) && matchesRule(rule, g, this.achievements));
  }

  loadCriteria(rule: SmartRule) {
    this.played = rule.played ?? 'any';
    this.ach = rule.achievements ?? 'any';
    this.minReview = rule.min_review;
    this.flagFilter = [...rule.flags];
    this.tagFilter = [...rule.tags];
  }

  visible = $derived.by(() => {
    const lib = this.library;
    if (!lib) return [] as Game[];
    const q = this.search.trim().toLowerCase();
    const now = Date.now() / 1000;
    const matchingTags = q ? Object.entries(lib.tags).filter(([, n]) => n.toLowerCase() === q).map(([id]) => Number(id)) : [];
    const list = lib.games.filter((g) => {
      if (this.view === 'none' ? g.collections.length > 0 : this.view !== 'all' && !g.collections.includes(this.view)) return false;
      if (q && !g.name.toLowerCase().includes(q) && !matchingTags.some((t) => g.tags.includes(t))) return false;
      return matchesRule(this.criteria, g, this.achievements, now);
    });
    const by: Record<Sort, (a: Game, b: Game) => number> = {
      // One shared collator is many times faster than calling localeCompare per comparison.
      name: (a, b) => COLLATOR.compare(a.name, b.name),
      playtime: (a, b) => b.playtime_minutes - a.playtime_minutes,
      recent: (a, b) => b.last_played - a.last_played,
      review: (a, b) => b.review_percent - a.review_percent,
    };
    return list.sort(by[this.sort]);
  });

  activeFilterCount = $derived(
    this.tagFilter.length + this.flagFilter.length + (this.played !== 'any' ? 1 : 0) + (this.ach !== 'any' ? 1 : 0) + (this.minReview > 0 ? 1 : 0),
  );

  uncategorizedCount = $derived((this.library?.games ?? []).filter((g) => g.collections.length === 0).length);

  /** For fast "is this game selected?" checks while drawing. */
  selectedSet = $derived(new Set(this.selected));

  selectedGames = $derived((this.library?.games ?? []).filter((g) => this.selectedSet.has(g.appid)));

  viewTitle = $derived(
    this.view === 'all' ? 'All games' : this.view === 'none' ? 'Not in any collection' : (this.collections.find((c) => c.id === this.view)?.name ?? 'Games'),
  );

  // ---- loading -----------------------------------------------------------------------

  async init() {
    this.loadTheme();
    this.status = await api.status();
    this.settings = await api.getSettings().catch(() => ({ automation: null }));
    this.settingsLoaded = true;
    await api.onOrganizer((e) => this.onOrganizer(e)).catch(() => {});
    if (this.status.ready) await this.refresh();
  }

  // ---- automation and settings -------------------------------------------------------

  /** What the background watcher reports. */
  private async onOrganizer(e: OrganizerEvent) {
    if (e.type === 'pending') {
      this.pending = e.ops;
    } else {
      if (e.snapshot) this.undoStack = [{ id: e.snapshot, label: 'auto-organize' }, ...this.undoStack];
      this.pending = null;
      await this.refreshQuietly().catch(() => {});
      this.say(`Organized ${e.applied} ${e.applied === 1 ? 'change' : 'changes'} for you`, !!e.snapshot);
    }
  }

  async setAutomation(mode: Automation) {
    try {
      this.settings = { ...this.settings, automation: mode };
      await api.setSettings($state.snapshot(this.settings) as Settings);
      if (mode !== 'notify') this.pending = null;
    } catch (e) {
      this.fail(e);
    }
  }

  /** Applies what the watcher offered. */
  async applyPending() {
    this.pending = null;
    await this.applyRules();
  }

  loadTheme() {
    let saved: string | null = null;
    try {
      saved = localStorage.getItem(THEME_KEY);
    } catch {
      /* private window or blocked storage: fall back to following the system */
    }
    this.theme = saved === 'light' || saved === 'dark' ? saved : 'system';
    this.applyTheme();
  }

  setTheme(theme: Theme) {
    this.theme = theme;
    try {
      localStorage.setItem(THEME_KEY, theme);
    } catch {
      /* not worth failing over */
    }
    this.applyTheme();
  }

  private applyTheme() {
    const root = document.documentElement;
    if (this.theme === 'system') root.removeAttribute('data-theme');
    else root.setAttribute('data-theme', THEME_NAMES[this.theme]);
  }

  async refresh() {
    this.loading = true;
    try {
      this.library = await api.loadLibrary();
      this.smartRules = await api.smartRules().catch(() => ({}));
      const ids = new Set(this.library.games.map((g) => g.appid));
      this.selected = this.selected.filter((id) => ids.has(id));
      if (this.view !== 'all' && this.view !== 'none' && !this.collections.some((c) => c.id === this.view)) this.view = 'all';
    } catch (e) {
      this.fail(e);
    } finally {
      this.loading = false;
    }
    // Achievement progress can take a few seconds the first time, so it fills in afterwards.
    api.loadAchievements().then((a) => (this.achievements = a)).catch(() => {});
  }

  // ---- selection ---------------------------------------------------------------------

  select(game: Game, e: { ctrlKey?: boolean; metaKey?: boolean; shiftKey?: boolean }) {
    const ids = this.visible.map((g) => g.appid);
    if (e.shiftKey && this.anchor !== null) {
      const [a, b] = [ids.indexOf(this.anchor), ids.indexOf(game.appid)];
      if (a >= 0 && b >= 0) this.selected = ids.slice(Math.min(a, b), Math.max(a, b) + 1);
    } else if (e.ctrlKey || e.metaKey) {
      this.selected = this.selected.includes(game.appid) ? this.selected.filter((x) => x !== game.appid) : [...this.selected, game.appid];
      this.anchor = game.appid;
    } else {
      this.selected = [game.appid];
      this.anchor = game.appid;
    }
  }

  selectAllVisible() {
    this.selected = this.visible.map((g) => g.appid);
  }

  clearSelection() {
    this.selected = [];
  }

  clearFilters() {
    this.tagFilter = [];
    this.flagFilter = [];
    this.played = 'any';
    this.ach = 'any';
    this.minReview = 0;
  }

  toggleTag(id: number) {
    this.tagFilter = this.tagFilter.includes(id) ? this.tagFilter.filter((t) => t !== id) : [...this.tagFilter, id];
  }

  toggleFlag(id: number) {
    this.flagFilter = this.flagFilter.includes(id) ? this.flagFilter.filter((t) => t !== id) : [...this.flagFilter, id];
  }

  // ---- changes -----------------------------------------------------------------------

  /** Runs a change, remembers its undo checkpoint, refreshes, and tells the user. */
  private async change(label: string, run: () => Promise<string | null | { snapshot: string | null; message: string }>, message: string) {
    if (this.busy) return false;
    this.busy = true;
    try {
      const result = await run();
      const checkpoint = result && typeof result === 'object' ? result.snapshot : result;
      if (checkpoint) this.undoStack = [{ id: checkpoint, label }, ...this.undoStack];
      await this.refreshQuietly();
      this.say(result && typeof result === 'object' ? result.message : message, !!checkpoint);
      return true;
    } catch (e) {
      this.fail(e);
      return false;
    } finally {
      this.busy = false;
    }
  }

  private async refreshQuietly() {
    this.library = await api.loadLibrary();
    this.smartRules = await api.smartRules().catch(() => this.smartRules);
    // The open collection may be gone (for example after undoing its creation).
    if (this.view !== 'all' && this.view !== 'none' && !this.collections.some((c) => c.id === this.view)) this.view = 'all';
  }

  // ---- smart collections -------------------------------------------------------------

  /** Creates a collection and fills it with every game the current filters match. */
  createSmart(name: string, auto: boolean) {
    const rule = $state.snapshot(this.criteria) as SmartRule;
    return this.change(
      `create ${name}`,
      async () => {
        const r = await api.createSmartCollection(name, rule, auto);
        return { snapshot: r.snapshot, message: `Created "${name}" with ${r.added} ${r.added === 1 ? 'game' : 'games'}` };
      },
      '',
    );
  }

  /** Adds games that fit the collection's saved criteria. */
  fillCollection(id: string) {
    const name = this.collections.find((c) => c.id === id)?.name ?? 'collection';
    return this.change(
      `fill ${name}`,
      async () => {
        const r = await api.fillCollection(id);
        return { snapshot: r.snapshot, message: r.added ? `Added ${r.added} ${r.added === 1 ? 'game' : 'games'} to ${name}` : `${name} is already up to date` };
      },
      '',
    );
  }

  /** Saves the current filters as a collection's auto-fill criteria, or clears them. */
  async saveRule(id: string, rule: SmartRule | null) {
    if (this.busy) return false;
    this.busy = true;
    try {
      await api.setSmartRule(id, rule ? ($state.snapshot(rule) as SmartRule) : null);
      this.smartRules = await api.smartRules();
      this.say(rule ? 'Auto-fill saved' : 'Auto-fill turned off', false);
      return true;
    } catch (e) {
      this.fail(e);
      return false;
    } finally {
      this.busy = false;
    }
  }

  openRuleDialog(collectionId: string) {
    this.ruleFor = collectionId;
    this.loadCriteria(this.smartRules[collectionId] ?? emptyRule());
    this.dialog = 'rule';
  }

  addToCollection(appids: number[], collectionId: string, move = false) {
    const name = this.collections.find((c) => c.id === collectionId)?.name ?? 'collection';
    const from = move && this.view !== 'all' && this.view !== 'none' && this.view !== collectionId ? [this.view] : [];
    const n = appids.length;
    return this.change(
      `add to ${name}`,
      () => api.moveGames(appids, [collectionId], from, `${from.length ? 'move' : 'add'} ${n} to ${name}`),
      `${from.length ? 'Moved' : 'Added'} ${n} ${n === 1 ? 'game' : 'games'} ${from.length ? 'to' : 'to'} ${name}`,
    );
  }

  removeFromCollection(appids: number[], collectionId: string) {
    const name = this.collections.find((c) => c.id === collectionId)?.name ?? 'collection';
    const n = appids.length;
    return this.change(
      `remove from ${name}`,
      () => api.moveGames(appids, [], [collectionId], `remove ${n} from ${name}`),
      `Removed ${n} ${n === 1 ? 'game' : 'games'} from ${name}`,
    );
  }

  toggleMembership(appid: number, collectionId: string) {
    const game = this.library?.games.find((g) => g.appid === appid);
    if (!game) return;
    return game.collections.includes(collectionId) ? this.removeFromCollection([appid], collectionId) : this.addToCollection([appid], collectionId);
  }

  createCollection(name: string, appids: number[] = []) {
    return this.change(`create ${name}`, () => api.createCollection(name, appids), `Created "${name}"`);
  }

  renameCollection(id: string, name: string) {
    return this.change(`rename to ${name}`, () => api.renameCollection(id, name), `Renamed to "${name}"`);
  }

  deleteCollection(id: string) {
    const name = this.collections.find((c) => c.id === id)?.name ?? 'collection';
    return this.change(`delete ${name}`, () => api.deleteCollection(id), `Deleted "${name}". Your games are still in your library.`);
  }

  async undo() {
    const last = this.undoStack[0];
    if (!last || this.busy) return;
    this.busy = true;
    try {
      await api.restoreSnapshot(last.id);
      this.undoStack = this.undoStack.slice(1);
      await this.refreshQuietly();
      this.say(`Undid: ${last.label}`, false);
    } catch (e) {
      this.fail(e);
    } finally {
      this.busy = false;
    }
  }

  async restore(snapshotId: string, label: string) {
    this.busy = true;
    try {
      await api.restoreSnapshot(snapshotId);
      await this.refreshQuietly();
      this.say(`Restored the collections from "${label}"`, false);
    } catch (e) {
      this.fail(e);
    } finally {
      this.busy = false;
    }
  }

  async applyRules() {
    // Whatever was waiting is being applied now, however the user got here.
    this.pending = null;
    await this.change('auto-organize', async () => (await api.applyRules()).snapshot, 'Your library is organized');
  }

  // ---- toasts ------------------------------------------------------------------------

  say(text: string, canUndo: boolean) {
    this.toast = { text, canUndo };
    clearTimeout(this.toastTimer);
    this.toastTimer = setTimeout(() => (this.toast = null), 7000);
  }

  fail(e: unknown) {
    this.toast = { text: String(e instanceof Error ? e.message : e), canUndo: false, error: true };
    clearTimeout(this.toastTimer);
    this.toastTimer = setTimeout(() => (this.toast = null), 10000);
  }
}

export const app = new AppStore();

export function fmtHours(minutes: number): string {
  if (minutes <= 0) return 'Never played';
  const h = minutes / 60;
  return h < 1 ? `${minutes} min` : `${h < 10 ? h.toFixed(1) : Math.round(h)} h`;
}

export function fmtAgo(ts: number): string {
  if (!ts) return 'Never';
  const days = Math.floor((Date.now() / 1000 - ts) / DAY);
  if (days <= 0) return 'Today';
  if (days === 1) return 'Yesterday';
  if (days < 30) return `${days} days ago`;
  if (days < 365) return `${Math.floor(days / 30)} mo ago`;
  return `${Math.floor(days / 365)} y ago`;
}
