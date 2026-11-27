import * as api from './api';
import type { Automation, DraftState, Game, Kind, OnlineProgress, Library, OpView, OrganizerEvent, Patch, Settings, SmartRule, Status } from './api';
import { countTests, editableGroup, emptyRule, matchesRule, readyCondition } from './criteria';
import type { Cond } from './criteria';
import { columnById, COLUMNS, DEFAULT_COLUMNS } from './columns';
import type { ColumnId, SortKey } from './columns';

export type PlayedFilter = 'any' | 'never' | 'played' | 'idle';
export type AchFilter = 'any' | 'complete' | 'incomplete';
export type Choice = 'any' | 'yes' | 'no';
/** 'new' creates a collection; 'rule' edits the auto-fill criteria of `ruleFor`. */
export type Dialog = null | 'organize' | 'history' | 'new' | 'rule' | 'settings' | 'save' | 'merge' | 'feedback' | 'presets';
export type Theme = 'system' | 'light' | 'dark';

const THEME_KEY = 'depiled-theme';
const COLUMNS_KEY = 'depiled-columns';

function loadColumns(): ColumnId[] {
  try {
    const saved = JSON.parse(localStorage.getItem(COLUMNS_KEY) ?? 'null');
    if (Array.isArray(saved)) return saved.filter((id) => COLUMNS.some((c) => c.id === id));
  } catch {
    /* blocked storage or a bad value: use the defaults */
  }
  return [...DEFAULT_COLUMNS];
}
const LAYOUT_KEY = 'depiled-layout';
function loadLayout(): 'grid' | 'compact' | 'list' {
  try {
    const v = localStorage.getItem(LAYOUT_KEY);
    if (v === 'grid' || v === 'compact' || v === 'list') return v;
  } catch {
    /* blocked storage: the default */
  }
  return 'grid';
}
export function saveLayout(v: string) {
  try {
    localStorage.setItem(LAYOUT_KEY, v);
  } catch {
    /* not worth failing over */
  }
}
// Numbers inside names sort as numbers ("Part 2" before "Part 10"), and case is ignored.
const COLLATOR = new Intl.Collator(undefined, { numeric: true, sensitivity: 'base' });
const THEME_NAMES = { light: 'shelf-day', dark: 'shelf-night' } as const;

/** Colours offered for collections. Picked to read well on both themes and on cover art, and
 *  kept clear of amber, which means unsaved changes. */
/** Collection colours, in rainbow order. No amber or yellow: amber means unsaved changes. Any
 *  other colour can be picked with the custom one. */
export const COLORS: { name: string; value: string }[] = [
  { name: 'Red', value: '#e5484d' },
  { name: 'Crimson', value: '#e93d82' },
  { name: 'Pink', value: '#d6409f' },
  { name: 'Plum', value: '#ab4aba' },
  { name: 'Violet', value: '#8e4ec6' },
  { name: 'Indigo', value: '#5b5bd6' },
  { name: 'Sky', value: '#0090ff' },
  { name: 'Cyan', value: '#00a2c7' },
  { name: 'Teal', value: '#12a594' },
  { name: 'Mint', value: '#3fb68b' },
  { name: 'Moss', value: '#46a758' },
  { name: 'Lime', value: '#86b31a' },
  { name: 'Olive', value: '#7d8b4e' },
  { name: 'Brown', value: '#ad7f58' },
  { name: 'Bronze', value: '#a18072' },
  { name: 'Slate', value: '#7d8596' },
];
/** Other things in the library besides games, each with its own place in the sidebar. */
export const OTHER_KINDS: { kind: Kind; label: string }[] = [
  { kind: 'demo', label: 'Demos' },
  { kind: 'playtest', label: 'Playtests' },
  { kind: 'software', label: 'Software' },
  { kind: 'tool', label: 'Tools' },
  { kind: 'soundtrack', label: 'Soundtracks' },
  { kind: 'video', label: 'Videos' },
  { kind: 'unknown', label: 'Unknown to Steam' },
];

/** In the All games view: Steam games and games added to Steam by hand. */
export const inGames = (g: Game) => g.kind === 'game' || g.kind === 'shortcut';
/** What rules and auto-fill look at, as in the Rust side's `is_game`. */
const ruleGame = (g: Game) => g.kind === 'game' && !g.family;

/** Collections without a colour use the muted text colour. */
export const NO_COLOR = 'var(--muted)';

interface Toast {
  text: string;
  undo?: boolean;
  error?: boolean;
}

const emptyDraft = (): DraftState => ({ pending: 0, changed: [], undo: null, redo: null, steam_running: false, rebased: false });


/** Text as search compares it: lower case, accents dropped, so "pokemon" finds "Pokémon". */
export const fold = (s: string) => s.normalize('NFD').replace(/\p{M}/gu, '').toLowerCase();

const searchTexts = new WeakMap<Game, string>();
/** A game's name and studios, folded. Edits replace a game's object, which drops its entry. */
function searchText(g: Game): string {
  let text = searchTexts.get(g);
  if (text === undefined) {
    // Newlines keep a search from matching across the end of one name and the start of the next.
    text = fold([g.name, ...g.developers, ...g.publishers].join('\n'));
    searchTexts.set(g, text);
  }
  return text;
}

class AppStore {
  status = $state<Status | null>(null);
  // `$state.raw` for the big data: replaced as a whole, never edited in place, and not
  // wrapping thousands of games in reactive proxies saves memory and time.
  library = $state.raw<Library | null>(null);
  draft = $state<DraftState>(emptyDraft());
  loading = $state(false);
  busy = $state(false);
  saving = $state(false);
  /** Whether the save in progress closes Steam first, so the status bar can say what's happening. */
  savingClosesSteam = $state(false);
  /** The last save this session, shown in the status bar until the next change. */
  lastSaved = $state<{ changes: number; at: number; verified: boolean } | null>(null);
  /** The online refresh in progress, if any. */
  online = $state<OnlineProgress | null>(null);
  private recheckOnline = false;

  search = $state('');
  /** 'all' (games), 'none' (games in no collection), 'kind:<kind>', or a collection id. */
  view = $state('all');
  tagFilter = $state<number[]>([]);
  flagFilter = $state<number[]>([]);
  played = $state<PlayedFilter>('any');
  ach = $state<AchFilter>('any');
  minReview = $state(0);
  /** Hours to unlock everything, by the SteamHunters median. 0 means any. */
  maxPerfectHours = $state(0);
  installed = $state<Choice>('any');
  /** Steam Deck categories to show: 1 unsupported, 2 playable, 3 verified, 0 unknown. */
  deck = $state<number[]>([]);
  /** Platform bits that must all be supported: 1 Windows, 2 macOS, 4 Linux. */
  platforms = $state(0);
  vr = $state<Choice>('any');
  earlyAccess = $state<Choice>('any');
  hasAchievements = $state<Choice>('any');
  /** Has screenshots, on this computer or the profile. */
  screenshots = $state<Choice>('any');
  family = $state<Choice>('any');
  yearFrom = $state(0);
  yearTo = $state(0);
  studio = $state('');
  onSale = $state<Choice>('any');
  /** Most to pay, in whole units of the region's currency. 0 means any. */
  maxPrice = $state(0);
  /** Fewest store reviews, to skip obscure games. 0 means any. */
  minReviews = $state(0);
  /** ProtonDB tiers to show. */
  proton = $state<string[]>([]);
  /** Conditions with AND, OR and NOT, built in the filter panel. */
  advanced = $state<Cond>({ all: [] });
  readyAdvanced = $derived(readyCondition(this.advanced));
  advancedCount = $derived(this.readyAdvanced ? countTests(this.readyAdvanced) : 0);
  unfinishedAdvanced = $derived(countTests(this.advanced) > this.advancedCount);

  /** The quick filters auto-fill rules have no fields for, as one condition. */
  quickCondition = $derived.by((): Cond[] => {
    const out: Cond[] = [];
    const yesNo = (field: 'installed' | 'vr' | 'early_access' | 'family' | 'on_sale', c: Choice) => c !== 'any' && out.push({ field, op: 'is', value: c === 'yes' });
    yesNo('installed', this.installed);
    yesNo('vr', this.vr);
    yesNo('early_access', this.earlyAccess);
    yesNo('family', this.family);
    yesNo('on_sale', this.onSale);
    if (this.screenshots !== 'any') out.push({ field: 'has_screenshots', op: 'is', value: this.screenshots === 'yes' });
    if (this.hasAchievements !== 'any') out.push({ field: 'has_achievements', op: 'is', value: this.hasAchievements === 'yes' });
    if (this.maxPerfectHours) out.push({ field: 'perfect_hours', op: 'lte', value: this.maxPerfectHours });
    if (this.deck.length) out.push({ field: 'deck', op: 'in', value: this.deck.map((d) => ['unknown', 'unsupported', 'playable', 'verified'][d]) });
    if (this.platforms) out.push({ field: 'platforms', op: 'has_all', value: [['windows', 1], ['mac', 2], ['linux', 4]].filter(([, b]) => this.platforms & (b as number)).map(([n]) => n) });
    if (this.yearFrom || this.yearTo) out.push({ field: 'release_year', op: 'between', value: [this.yearFrom || null, this.yearTo || null] });
    if (this.studio.trim()) out.push({ any: [{ field: 'developer', op: 'contains', value: this.studio.trim() }, { field: 'publisher', op: 'contains', value: this.studio.trim() }] });
    if (this.maxPrice) out.push({ field: 'price', op: 'lte', value: this.maxPrice });
    if (this.minReviews) out.push({ field: 'review_count', op: 'gte', value: this.minReviews });
    if (this.proton.length) out.push({ field: 'proton', op: 'in', value: this.proton });
    return out;
  });
  sort = $state<SortKey>('name');
  sortDesc = $state(false);
  columns = $state<ColumnId[]>(loadColumns());
  /** Large covers, small covers (more per screen for big libraries), or the table. */
  layout = $state<'grid' | 'compact' | 'list'>(loadLayout());

  selected = $state.raw<number[]>([]);
  anchor: number | null = null;

  settings = $state<Settings>({
    automation: null,
    online_stats: null,
    region: null,
    online_achievements: true,
    online_prices: true,
    online_protondb: true,
    online_hunters: true,
    reopen_steam: true,
    check_updates: true,
    update_channel: 'stable',
    account: null,
  });
  /** A crash to ask about: the reason this window opened (`crashWindow`), or one left over
   *  from an earlier run. */
  crash = $state<api.Crash | null>(null);
  crashWindow = $state(false);
  canSendReports = $state(false);
  /** What the feedback dialog starts with, e.g. an error message from a Report button. */
  feedbackSeed = $state<string | null>(null);
  /** The last error messages shown or thrown, for bug reports. Not reactive: read on demand. */
  recentErrors: string[] = [];
  /** The user keeps their own collection order. */
  ordered = $state(false);
  /** The Steam accounts on this computer and which one is shown. */
  accounts = $state.raw<api.Accounts | null>(null);
  currentAccount = $derived(this.accounts?.accounts.find((a) => a.id === this.accounts?.current) ?? null);
  /** This build's version. */
  version = $state('');
  /** A newer version that was found, and how far its download is. */
  update = $state<{ info: api.UpdateInfo; phase: 'ready' | 'downloading' | 'installing'; downloaded: number; total: number | null } | null>(null);
  checkingUpdate = $state(false);
  /** Country for store prices: the user's choice, or this computer's region. */
  region = $derived(this.settings.region ?? this.library?.online.detected_region ?? 'US');
  settingsLoaded = $state(false);
  promptDismissed = $state(false);
  /** Changes the watcher found and is waiting for the user to approve. */
  pending = $state<OpView[] | null>(null);
  theme = $state<Theme>('system');

  /** Asked once, the first time, how automatic things should be. */
  showAutomationPrompt = $derived(this.settingsLoaded && this.settings.automation === null && !this.promptDismissed);
  showStatsPrompt = $derived(this.settingsLoaded && this.settings.automation !== null && this.settings.online_stats === null && !this.promptDismissed);

  dialog = $state<Dialog>(null);
  /** Collection the merge dialog merges into. */
  mergeInto = $state<string | null>(null);
  toast = $state<Toast | null>(null);
  private toastTimer: ReturnType<typeof setTimeout> | undefined;

  collections = $derived(this.library?.collections ?? []);
  tagNames = $derived(this.library?.tags ?? {});
  colorOf = $derived(new Map(this.collections.map((c) => [c.id, c.color ?? NO_COLOR])));
  changedSet = $derived(new Set(this.draft.changed));

  /** Tags worth offering as filters: common ones first. */
  tagChoices = $derived.by(() => {
    const counts = new Map<number, number>();
    for (const g of this.library?.games ?? []) for (const t of g.tags) counts.set(t, (counts.get(t) ?? 0) + 1);
    return [...counts.entries()]
      .sort((a, b) => b[1] - a[1])
      .map(([id, n]) => ({ id, name: this.tagNames[id] ?? String(id), count: n }));
  });

  achievementsKnown = $derived((this.library?.games ?? []).filter((g) => g.achievements).length);
  huntersKnown = $derived((this.library?.games ?? []).filter((g) => g.hunters && g.hunters.median_minutes > 0).length);

  /** The current filters as a rule, ignoring the search box and which collection is open. */
  criteria = $derived<SmartRule>({
    played: this.played === 'any' ? null : this.played,
    achievements: this.ach === 'any' ? null : this.ach,
    min_review: this.minReview,
    flags: this.flagFilter,
    tags: this.tagFilter,
    condition: this.quickCondition.length ? { all: [...this.quickCondition, ...(this.readyAdvanced ? [this.readyAdvanced] : [])] } : this.readyAdvanced,
  });

  /** Every game the current filters match, wherever it is filed. Like auto-fill itself, only
   *  games the account owns: Family games come and go as their owner plays them. */
  criteriaMatches = $derived((this.library?.games ?? []).filter((g) => ruleGame(g) && matchesRule(this.criteria, g)));

  /** What each collection auto-fills with, by collection id. */
  smartRules = $state.raw<Record<string, SmartRule>>({});
  /** Collection whose criteria the 'rule' dialog edits. */
  ruleFor = $state<string | null>(null);

  /** Games that fit a collection's saved criteria but aren't in it yet. */
  newMatches(collectionId: string): Game[] {
    const rule = this.smartRules[collectionId];
    if (!rule) return [];
    return (this.library?.games ?? []).filter((g) => ruleGame(g) && !g.collections.includes(collectionId) && matchesRule(rule, g));
  }

  loadCriteria(rule: SmartRule) {
    this.played = rule.played ?? 'any';
    this.ach = rule.achievements ?? 'any';
    this.minReview = rule.min_review;
    this.flagFilter = [...rule.flags];
    this.tagFilter = [...rule.tags];
    this.advanced = rule.condition ? editableGroup(structuredClone(rule.condition)) : { all: [] };
  }

  /** Each game's search text, in library order. Built on the first search and again when the
   *  library changes, reusing the text of every game that didn't; a keystroke is then one
   *  substring test per string, which is quicker than looking each game up. */
  private searchIndex = $derived((this.library?.games ?? []).map(searchText));

  visible = $derived.by(() => {
    const lib = this.library;
    if (!lib) return [] as Game[];
    const q = fold(this.search.trim());
    const now = Date.now() / 1000;
    const matchingTags = q ? Object.entries(lib.tags).filter(([, n]) => fold(n) === q).map(([id]) => Number(id)) : [];
    const rule = this.criteria;
    const index = q ? this.searchIndex : null;
    const list = lib.games.filter((g, i) => {
      if (!this.inView(g)) return false;
      if (index && !index[i].includes(q) && !matchingTags.some((t) => g.tags.includes(t))) return false;
      return matchesRule(rule, g, now);
    });
    // Unknown values sort last whatever the direction; ties fall back to the name.
    const key = this.sort;
    const value = key === 'name' ? (g: Game) => g.name : columnById(key).value;
    const dir = this.sortDesc ? -1 : 1;
    const decorated = list.map((g) => ({ g, v: value(g) }));
    decorated.sort((a, b) => {
      if (a.v === null || b.v === null) return a.v === b.v ? COLLATOR.compare(a.g.name, b.g.name) : a.v === null ? 1 : -1;
      const c = typeof a.v === 'string' ? COLLATOR.compare(a.v, b.v as string) : a.v - (b.v as number);
      return c ? c * dir : COLLATOR.compare(a.g.name, b.g.name);
    });
    return decorated.map((d) => d.g);
  });

  activeFilterCount = $derived(
    this.tagFilter.length +
      this.flagFilter.length +
      (this.played !== 'any' ? 1 : 0) +
      (this.ach !== 'any' ? 1 : 0) +
      (this.minReview > 0 ? 1 : 0) +
      (this.maxPerfectHours > 0 ? 1 : 0) +
      [this.installed, this.vr, this.earlyAccess, this.hasAchievements, this.family, this.screenshots].filter((c) => c !== 'any').length +
      (this.deck.length ? 1 : 0) +
      (this.platforms ? 1 : 0) +
      (this.yearFrom || this.yearTo ? 1 : 0) +
      (this.studio.trim() ? 1 : 0) +
      (this.onSale !== 'any' ? 1 : 0) +
      (this.maxPrice ? 1 : 0) +
      (this.minReviews ? 1 : 0) +
      (this.proton.length ? 1 : 0) +
      this.advancedCount,
  );

  uncategorizedCount = $derived((this.library?.games ?? []).filter((g) => inGames(g) && g.collections.length === 0).length);
  gameCount = $derived((this.library?.games ?? []).filter(inGames).length);
  kindCounts = $derived.by(() => {
    const counts: Partial<Record<Kind, number>> = {};
    for (const g of this.library?.games ?? []) counts[g.kind] = (counts[g.kind] ?? 0) + 1;
    return counts;
  });

  /** Whether a game belongs in the open view. Collections show everything filed in them. */
  inView(g: Game): boolean {
    const v = this.view;
    if (v === 'all') return inGames(g);
    if (v === 'none') return inGames(g) && g.collections.length === 0;
    if (v.startsWith('kind:')) return g.kind === v.slice(5);
    return g.collections.includes(v);
  }

  /** For fast "is this game selected?" checks while drawing. */
  selectedSet = $derived(new Set(this.selected));
  /** The open view is one of the user's collections. */
  isCollectionView = $derived(this.view !== 'all' && this.view !== 'none' && !this.view.startsWith('kind:'));
  selectedGames = $derived((this.library?.games ?? []).filter((g) => this.selectedSet.has(g.appid)));

  viewTitle = $derived(
    this.view === 'all'
      ? 'All games'
      : this.view === 'none'
        ? 'Not in a collection'
        : this.view.startsWith('kind:')
          ? (OTHER_KINDS.find((k) => 'kind:' + k.kind === this.view)?.label ?? 'Games')
          : (this.collections.find((c) => c.id === this.view)?.name ?? 'Games'),
  );

  // ---- loading -----------------------------------------------------------------------

  async init() {
    this.loadTheme();
    this.canSendReports = await api.canSendReports().catch(() => false);
    // Opened by a crash: this window only asks about the report.
    const crashed = await api.crashMode().catch(() => null);
    if (crashed) {
      this.crash = crashed;
      this.crashWindow = true;
      return;
    }
    this.watchErrors();
    this.status = await api.status();
    this.settings = await api.getSettings().catch(() => this.settings);
    this.settingsLoaded = true;
    await api.onOrganizer((e) => this.onOrganizer(e)).catch(() => {});
    await api.onOnline((e) => this.onOnline(e)).catch(() => {});
    await api.onOnlineSaved(() => this.showSavedOnline()).catch(() => {});
    await api.onUpdateProgress((p) => this.onUpdateProgress(p)).catch(() => {});
    this.version = await api.appVersion().catch(() => '');
    // Once soon after starting, out of the way of reading the library, then twice a day for
    // people who leave the app open.
    if (api.isDesktop) {
      setTimeout(() => this.settings.check_updates && this.checkUpdate(), 8000);
      setInterval(() => this.settings.check_updates && this.checkUpdate(), 12 * 3600 * 1000);
    }
    if (this.status.steam_found) {
      await this.refresh();
      this.loadAccounts();
      // A crash the reporter window didn't get to ask about.
      api.pendingCrashes().then((c) => (this.crash = c[0] ?? null)).catch(() => {});
      this.refreshOnline();
    }
  }

  async refresh() {
    this.loading = true;
    try {
      this.setLibrary(await api.loadLibrary());
      this.smartRules = await api.smartRules().catch(() => ({}));
    } catch (e) {
      this.fail(e);
    } finally {
      this.loading = false;
    }
  }

  private setLibrary(lib: Library) {
    this.library = lib;
    this.ordered = lib.ordered;
    this.draft = lib.draft;
    const ids = new Set(lib.games.map((g) => g.appid));
    this.selected = this.selected.filter((id) => ids.has(id));
    this.keepView();
  }

  /** The open collection may be gone (deleted, merged, or undone). */
  private keepView() {
    if (this.isCollectionView && !this.collections.some((c) => c.id === this.view)) this.view = 'all';
  }

  /** Applies what an edit changed, without reloading the library. */
  private applyPatch(p: Patch) {
    const lib = this.library;
    if (!lib) return;
    const moved = p.membership;
    const games = Object.keys(moved).length ? lib.games.map((g) => (g.appid in moved ? { ...g, collections: moved[g.appid] } : g)) : lib.games;
    this.library = { ...lib, games, collections: p.collections, draft: p.draft };
    this.draft = p.draft;
    this.keepView();
  }

  // ---- background events -------------------------------------------------------------

  private async onOrganizer(e: OrganizerEvent) {
    if (e.type === 'changed') {
      // Steam's files moved on (a game was played, or Steam saved). Reload quietly.
      this.draft = { ...this.draft, steam_running: e.steam_running };
      if (!this.busy && !this.saving) await api.loadLibrary().then((l) => this.setLibrary(l)).catch(() => {});
      // A new game, or one just played, gets its online details straight away.
      if (this.online) this.recheckOnline = true;
      else this.refreshOnline();
    } else if (e.type === 'account') {
      // Someone else signed in to Steam, and Depiled follows Steam's sign-in.
      await this.afterAccountChange();
      if (this.currentAccount) this.say(`${this.currentAccount.name || this.currentAccount.login} signed in to Steam, so this is their library now.`, false);
    } else if (e.type === 'pending') {
      this.pending = e.ops;
    } else {
      this.pending = null;
      await api.loadLibrary().then((l) => this.setLibrary(l)).catch(() => {});
      this.say(e.saved ? `Organized ${e.applied} ${e.applied === 1 ? 'change' : 'changes'} and saved them to Steam` : `Organized ${e.applied} ${e.applied === 1 ? 'change' : 'changes'}. They will be saved when Steam closes.`, false);
    }
  }

  private async onOnline(e: OnlineProgress) {
    this.online = e.finished ? null : e;
    if (e.finished) {
      if (e.errors.length) this.say(e.errors.join(' '), false, true);
      // Only the online caches changed, so Steam's own files aren't read again.
      if (e.changed !== false) await api.loadLibrary(true).then((l) => this.setLibrary(l)).catch(() => {});
      if (this.recheckOnline) {
        this.recheckOnline = false;
        this.refreshOnline();
      }
    }
  }

  /** Shows a source's data as soon as it is saved. Only the online caches are read again. */
  private async showSavedOnline() {
    if (this.busy || this.saving) return;
    await api.loadLibrary(true).then((l) => this.setLibrary(l)).catch(() => {});
  }

  stopOnline() {
    this.recheckOnline = false;
    api.stopOnline().catch((e) => this.fail(e));
  }

  /** Fetches whatever online extras are due (new games, a week or a day old); everything
   *  with `force`. Does nothing while offline extras are switched off. */
  async refreshOnline(force = false) {
    if (!this.settings.online_stats || this.online) return;
    this.online = { task: 'online details', done: 0, total: 0, finished: false, errors: [] };
    try {
      // Set pending before IPC: a cache-only worker can finish before invoke resolves.
      const started = await api.refreshOnline(this.region, force);
      if (!started && !this.settings.online_stats) this.online = null;
    } catch (e) {
      this.online = null;
      this.fail(e);
    }
  }
  // ---- accounts ----------------------------------------------------------------------

  async loadAccounts() {
    this.accounts = await api.accounts().catch(() => this.accounts);
  }

  /** Shows another account's library, or (null) whoever is signed in to Steam. Each account's
   *  unsaved changes, history and rules stay with it. */
  async switchAccount(account: number | null) {
    if (this.busy || this.saving) return;
    const before = this.currentAccount;
    const pending = this.draft.pending;
    this.busy = true;
    try {
      await api.setAccount(account);
      this.settings = { ...this.settings, account };
      await this.afterAccountChange();
      if (pending && before && before.id !== this.currentAccount?.id) {
        this.say(`${before.name || before.login}’s ${pending} unsaved ${pending === 1 ? 'change is' : 'changes are'} kept for when you switch back.`, false);
      }
    } catch (e) {
      this.fail(e);
    } finally {
      this.busy = false;
    }
  }

  private async afterAccountChange() {
    this.pending = null;
    this.selected = [];
    this.view = 'all';
    this.lastSaved = null;
    await this.refresh();
    await this.loadAccounts();
    // The new account's achievement progress comes from its own profile.
    if (this.online) this.recheckOnline = true;
    else this.refreshOnline();
  }

  // ---- updates -----------------------------------------------------------------------

  /** Looks for a newer version. An automatic check stays quiet when offline or up to date. */
  async checkUpdate(manual = false) {
    if (this.checkingUpdate || (this.update && this.update.phase !== 'ready')) return;
    this.checkingUpdate = true;
    try {
      const info = await api.checkUpdate(this.settings.update_channel === 'nightly');
      this.update = info ? { info, phase: 'ready', downloaded: 0, total: null } : null;
      if (manual && !info) this.say(`Depiled is up to date${this.version ? ` (${this.version})` : ''}.`, false);
    } catch (e) {
      if (manual) this.say(`Couldn’t check for updates. ${e instanceof Error ? e.message : e}`, false, true);
    } finally {
      this.checkingUpdate = false;
    }
  }

  /** Downloads the update and restarts into it. Unsaved changes are kept: they're on disk. */
  async installUpdate() {
    const u = this.update;
    if (!u || u.phase !== 'ready') return;
    this.update = { ...u, phase: 'downloading' };
    try {
      await api.installUpdate();
    } catch (e) {
      this.update = { ...u, phase: 'ready' };
      this.fail(e);
    }
  }

  private onUpdateProgress(p: api.UpdateProgress) {
    if (!this.update) return;
    const done = p.total !== null && p.downloaded >= p.total;
    this.update = { ...this.update, downloaded: p.downloaded, total: p.total, phase: done ? 'installing' : 'downloading' };
  }

  // ---- settings ----------------------------------------------------------------------

  async updateSettings(change: Partial<Settings>) {
    try {
      this.settings = { ...this.settings, ...change };
      await api.setSettings($state.snapshot(this.settings) as Settings);
      if (change.automation && change.automation !== 'notify') this.pending = null;
      if (change.update_channel && this.update?.phase !== 'downloading') {
        this.update = null;
        this.checkUpdate();
      }
      if (change.region !== undefined || Object.keys(change).some((k) => k.startsWith('online_'))) {
        if (this.online) {
          this.recheckOnline = this.settings.online_stats === true;
          await api.stopOnline();
        } else if (this.settings.online_stats) {
          this.refreshOnline();
        }
      }
    } catch (e) {
      this.fail(e);
    }
  }

  setAutomation(mode: Automation) {
    return this.updateSettings({ automation: mode });
  }

  async disableDebugPort() {
    try {
      await api.disableDebugPort();
      this.status = await api.status();
      this.say("Steam's debug port is switched off. It takes effect the next time Steam starts.", false);
    } catch (e) {
      this.fail(e);
    }
  }

  loadTheme() {
    let saved: string | null = null;
    try {
      saved = localStorage.getItem(THEME_KEY);
    } catch {
      /* private window or blocked storage: follow the system */
    }
    // The browser demo takes ?theme=light or ?theme=dark, for screenshots of either theme.
    if (!api.isDesktop) saved = new URLSearchParams(location.search).get('theme') ?? saved;
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

  // ---- selection and filters ---------------------------------------------------------

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
    this.maxPerfectHours = 0;
    this.installed = this.vr = this.earlyAccess = this.hasAchievements = this.family = this.screenshots = 'any';
    this.deck = [];
    this.platforms = 0;
    this.yearFrom = this.yearTo = 0;
    this.studio = '';
    this.onSale = 'any';
    this.maxPrice = this.minReviews = 0;
    this.proton = [];
    this.advanced = { all: [] };
  }

  /** Sorts by key in its natural direction. */
  setSort(key: SortKey) {
    this.sort = key;
    this.sortDesc = key === 'name' ? false : columnById(key).desc;
  }

  /** Sorts by a column; choosing the same column again flips the direction. */
  sortBy(key: SortKey) {
    if (this.sort === key) this.sortDesc = !this.sortDesc;
    else {
      this.sort = key;
      this.sortDesc = key === 'name' ? false : columnById(key).desc;
    }
  }

  toggleColumn(id: ColumnId) {
    this.columns = this.columns.includes(id) ? this.columns.filter((c) => c !== id) : COLUMNS.map((c) => c.id).filter((c) => c === id || this.columns.includes(c));
    try {
      localStorage.setItem(COLUMNS_KEY, JSON.stringify(this.columns));
    } catch {
      /* not worth failing over */
    }
  }

  toggleTag(id: number) {
    this.tagFilter = this.tagFilter.includes(id) ? this.tagFilter.filter((t) => t !== id) : [...this.tagFilter, id];
  }

  toggleFlag(id: number) {
    this.flagFilter = this.flagFilter.includes(id) ? this.flagFilter.filter((t) => t !== id) : [...this.flagFilter, id];
  }

  // ---- edits -------------------------------------------------------------------------

  /** Runs an edit on the draft and shows what happened. Edits are instant and stay in the
   *  app until saved, so there is nothing to wait for. */
  private async edit(run: () => Promise<Patch>, message: (p: Patch) => string | null) {
    if (this.busy) return null;
    this.busy = true;
    try {
      const p = await run();
      this.applyPatch(p);
      const text = message(p);
      if (text) this.say(text, true);
      return p;
    } catch (e) {
      this.fail(e);
      return null;
    } finally {
      this.busy = false;
    }
  }

  private nameOf = (id: string) => this.collections.find((c) => c.id === id)?.name ?? 'collection';
  private games = (n: number) => `${n} ${n === 1 ? 'game' : 'games'}`;

  addToCollection(appids: number[], collectionId: string, move = false) {
    const name = this.nameOf(collectionId);
    const from = move && this.isCollectionView && this.view !== collectionId ? [this.view] : [];
    const n = appids.length;
    return this.edit(
      () => api.moveGames(appids, [collectionId], from, `${from.length ? 'move' : 'add'} ${this.games(n)} to ${name}`),
      () => `${from.length ? 'Moved' : 'Added'} ${this.games(n)} to ${name}`,
    );
  }

  removeFromCollection(appids: number[], collectionId: string) {
    const name = this.nameOf(collectionId);
    return this.edit(() => api.moveGames(appids, [], [collectionId], `take ${this.games(appids.length)} out of ${name}`), () => `Took ${this.games(appids.length)} out of ${name}`);
  }

  toggleMembership(appid: number, collectionId: string) {
    const game = this.library?.games.find((g) => g.appid === appid);
    if (!game) return;
    return game.collections.includes(collectionId) ? this.removeFromCollection([appid], collectionId) : this.addToCollection([appid], collectionId);
  }

  async createCollection(name: string, appids: number[] = []) {
    const p = await this.edit(() => api.createCollection(name, appids), () => `Created ${name.trim()}`);
    return p?.value ?? null;
  }

  async setOrdering(on: boolean) {
    const p = await this.edit(() => api.setOrdering(on), () => (on ? 'Numbered your collections. Drag them in the sidebar to reorder.' : 'Took the numbers off your collections.'));
    if (p) this.ordered = on;
  }

  /** Moves a collection to just before another (or to the end with null). */
  moveCollection(id: string, before: string | null) {
    const ids = this.collections.filter((c) => !c.dynamic && c.id !== id).map((c) => c.id);
    const at = before ? ids.indexOf(before) : -1;
    ids.splice(at < 0 ? ids.length : at, 0, id);
    return this.edit(() => api.setOrder(ids), () => null);
  }

  /** Moves a collection one place up or down. */
  nudgeCollection(id: string, by: -1 | 1) {
    const ids = this.collections.filter((c) => !c.dynamic).map((c) => c.id);
    const i = ids.indexOf(id);
    const j = i + by;
    if (i < 0 || j < 0 || j >= ids.length) return;
    [ids[i], ids[j]] = [ids[j], ids[i]];
    return this.edit(() => api.setOrder(ids), () => null);
  }

  renameCollection(id: string, name: string) {
    return this.edit(() => api.renameCollection(id, name), () => `Renamed to ${name.trim()}`);
  }

  deleteCollection(id: string) {
    const name = this.nameOf(id);
    return this.edit(() => api.deleteCollection(id), () => `Deleted ${name}. The games are still in your library.`);
  }

  async duplicateCollection(id: string) {
    const p = await this.edit(() => api.duplicateCollection(id), (p) => `Made ${this.nameOf(p.value ?? '')}`);
    if (p?.value) this.view = p.value;
  }

  mergeCollections(sources: string[], into: string) {
    const name = this.nameOf(into);
    return this.edit(() => api.mergeCollections(sources, into), () => `Merged ${sources.length} ${sources.length === 1 ? 'collection' : 'collections'} into ${name}`);
  }

  setColor(id: string, color: string | null) {
    return this.edit(() => api.setColor(id, color), () => null);
  }

  private colorTimer: ReturnType<typeof setTimeout> | undefined;
  /** A colour while it's being picked on the colour wheel: shown at once everywhere, and
   *  stored once the wheel has been still for a moment. Colours aren't part of what's saved to
   *  Steam, so this makes no undo steps. */
  liveColor(id: string, color: string) {
    const lib = this.library;
    if (!lib) return;
    this.library = { ...lib, collections: lib.collections.map((c) => (c.id === id ? { ...c, color } : c)) };
    clearTimeout(this.colorTimer);
    this.colorTimer = setTimeout(() => api.setColor(id, color).catch((e) => this.fail(e)), 200);
  }

  undo() {
    if (!this.draft.undo) return;
    const label = this.draft.undo;
    return this.edit(() => api.undo(), () => `Undid: ${label}`);
  }

  redo() {
    if (!this.draft.redo) return;
    const label = this.draft.redo;
    return this.edit(() => api.redo(), () => `Redid: ${label}`);
  }

  discard() {
    return this.edit(() => api.discard(), () => 'Discarded the changes that weren’t in Steam yet');
  }

  restore(snapshotId: string) {
    return this.edit(() => api.restoreSnapshot(snapshotId), () => 'Restored the earlier version. Save to put it back in Steam.');
  }

  async applyRules() {
    this.pending = null;
    await this.edit(() => api.applyRules(), (p) => (Number(p.value) ? `Organized ${p.value} ${p.value === '1' ? 'change' : 'changes'}` : 'Your library already follows your rules'));
  }

  async applyPending() {
    await this.applyRules();
  }

  /** Writes the draft to Steam. With `closeSteam`, Steam is closed first and reopened after. */
  async save(closeSteam: boolean) {
    if (this.saving) return false;
    this.saving = true;
    try {
      this.savingClosesSteam = closeSteam;
      const r = await api.save(closeSteam);
      this.lastSaved = { changes: r.changes, at: Date.now(), verified: r.verified };
      this.setLibrary(await api.loadLibrary());
      // A good save is confirmed in the status bar, where it stays; only a mismatch needs a toast.
      if (!r.verified) this.say('Saved, but Steam’s file doesn’t match exactly. Open History to compare or restore.', false, true);
      return true;
    } catch (e) {
      this.fail(e);
      return false;
    } finally {
      this.saving = false;
      this.status = await api.status().catch(() => this.status);
    }
  }

  // ---- smart collections -------------------------------------------------------------

  async createSmart(name: string, auto: boolean) {
    const rule = $state.snapshot(this.criteria) as SmartRule;
    const p = await this.edit(
      () => api.createSmartCollection(name, rule, auto),
      (p) => `Created ${name.trim()} with ${this.games(Number(p.value?.split(':')[1] ?? 0))}`,
    );
    if (p && auto) this.smartRules = await api.smartRules().catch(() => this.smartRules);
    return p?.value?.split(':')[0] ?? null;
  }

  fillCollection(id: string) {
    const name = this.nameOf(id);
    return this.edit(() => api.fillCollection(id), (p) => (Number(p.value) ? `Added ${this.games(Number(p.value))} to ${name}` : `${name} is already up to date`));
  }

  async saveRule(id: string, rule: SmartRule | null) {
    try {
      await api.setSmartRule(id, rule ? ($state.snapshot(rule) as SmartRule) : null);
      this.smartRules = await api.smartRules();
      this.say(rule ? 'Auto-fill saved' : 'Auto-fill turned off', false);
      return true;
    } catch (e) {
      this.fail(e);
      return false;
    }
  }

  private restoreFilters: (() => void) | null = null;

  private rememberFilters() {
    const keys = ['played', 'ach', 'minReview', 'flagFilter', 'tagFilter', 'advanced', 'maxPerfectHours',
      'installed', 'vr', 'earlyAccess', 'hasAchievements', 'family', 'screenshots', 'deck', 'platforms', 'yearFrom',
      'yearTo', 'studio', 'onSale', 'maxPrice', 'minReviews', 'proton'] as const;
    const saved = Object.fromEntries(keys.map((key) => [key, $state.snapshot(this[key])]));
    this.restoreFilters = () => Object.assign(this, saved);
  }

  openNewCollection() {
    this.rememberFilters();
    this.dialog = 'new';
  }

  closeRuleEditor(restore: boolean) {
    if (restore) this.restoreFilters?.();
    this.restoreFilters = null;
    this.ruleFor = null;
    this.dialog = null;
  }

  openRuleDialog(collectionId: string) {
    this.rememberFilters();
    this.clearFilters();
    this.ruleFor = collectionId;
    this.loadCriteria(this.smartRules[collectionId] ?? emptyRule());
    this.dialog = 'rule';
  }

  openMerge(into: string) {
    this.mergeInto = into;
    this.dialog = 'merge';
  }

  // ---- toasts ------------------------------------------------------------------------

  // ---- presets -----------------------------------------------------------------------

  /** Sets up a preset's collections. True when it worked. */
  async applyPreset(id: string): Promise<boolean> {
    this.busy = true;
    try {
      const r = await api.applyPreset(id);
      this.applyPatch(r.patch);
      this.smartRules = await api.smartRules().catch(() => this.smartRules);
      const list = (names: string[]) => (names.length > 1 ? `${names.slice(0, -1).join(', ')} and ${names[names.length - 1]}` : names[0]);
      const parts: string[] = [];
      if (r.created.length) parts.push(`Made ${list(r.created)}`);
      if (r.attached.length) parts.push(`${list(r.attached)} now ${r.attached.length === 1 ? 'fills' : 'fill'} itself`);
      if (r.kept.length) parts.push(`${list(r.kept)} kept ${r.kept.length === 1 ? 'its' : 'their'} own criteria`);
      this.say(parts.length ? parts.join('. ') + '.' : 'You already have all of these.', parts.length > 0);
      return true;
    } catch (e) {
      this.fail(e);
      return false;
    } finally {
      this.busy = false;
    }
  }

  // ---- feedback ----------------------------------------------------------------------

  openFeedback(seed: string | null = null) {
    this.feedbackSeed = seed;
    this.toast = null;
    this.dialog = 'feedback';
  }

  private noteError(text: string) {
    this.recentErrors.push(`${new Date().toISOString().slice(11, 19)} ${text}`.slice(0, 500));
    if (this.recentErrors.length > 20) this.recentErrors.shift();
  }

  /** Remembers errors the window hits on its own (not just ones shown), for bug reports. */
  private watchErrors() {
    window.addEventListener('error', (e) => this.noteError(`window: ${e.message} (${e.filename?.split('/').pop()}:${e.lineno})`));
    window.addEventListener('unhandledrejection', (e) => this.noteError(`window: ${e.reason instanceof Error ? e.reason.message : String(e.reason)}`));
  }

  say(text: string, undo: boolean, error = false) {
    if (error) this.noteError(text);
    this.toast = { text, undo, error };
    clearTimeout(this.toastTimer);
    this.toastTimer = setTimeout(() => (this.toast = null), error ? 10000 : 6000);
  }

  fail(e: unknown) {
    this.say(String(e instanceof Error ? e.message : e), false, true);
  }
}

export const app = new AppStore();

export { fmtAgo, fmtHours, fmtSize, fmtSpan, fmtYear } from './format';
