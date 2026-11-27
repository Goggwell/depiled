import { invoke, convertFileSrc } from '@tauri-apps/api/core';
import type { Cond } from './criteria';

export interface Achievements {
  unlocked: number;
  total: number;
}

/** Completion statistics from SteamHunters, cached on disk. Times are in minutes. */
export interface HunterStats {
  median_minutes: number;
  fastest_minutes: number;
  perfected: number;
  players: number;
  achievements: number;
}

/** What sort of library entry something is. Rules only ever look at games. */
export type Kind = 'game' | 'demo' | 'playtest' | 'software' | 'tool' | 'soundtrack' | 'video' | 'shortcut' | 'unknown';

/** Store data for the chosen region, cached on disk. Prices are in cents. */
export interface StoreInfo {
  price_cents: number | null;
  price_text: string | null;
  discount: number;
  discount_ends: number;
  review_count: number;
  review_label: string;
  fetched: number;
}

export interface OnlineState {
  /** Country the cached prices are for. */
  country: string;
  detected_region: string | null;
  profile_private: boolean;
  achievements_read: number;
  usage: Usage;
  /** Regions whose prices are kept, so switching to them is instant. */
  cached_regions: string[];
  sources: SourceState[];
}

export interface SourceState {
  /** The setting that switches it on or off. */
  key: 'online_achievements' | 'online_prices' | 'online_hunters' | 'online_protondb';
  label: string;
  updated: number;
  items: number;
}

/** What the online extras downloaded. */
export interface Usage {
  month: string;
  month_requests: number;
  month_bytes: number;
  total_requests: number;
  total_bytes: number;
  last_run: number;
}

export interface OnlineProgress {
  changed?: boolean;
  /** What is being fetched, in words. Sources run side by side, so this can name several. */
  task: string;
  /** Each source still fetching, by its task name. */
  active?: string[];
  done: number;
  total: number;
  finished: boolean;
  errors: string[];
}

/** Store details read from Steam's cache. Zero means unknown. */
export interface Extras {
  /** Steam Deck: 1 unsupported, 2 playable, 3 verified. */
  deck: number;
  /** 1 partial, 2 full controller support. */
  controller: number;
  /** Bits: 1 Windows, 2 macOS, 4 Linux. */
  platforms: number;
  vr: boolean;
  early_access: boolean;
}

export interface Game {
  appid: number;
  name: string;
  kind: Kind;
  playtime_minutes: number;
  last_played: number;
  categories: number[];
  tags: number[];
  metacritic: number;
  review_percent: number;
  release: number;
  developers: string[];
  publishers: string[];
  achievements: Achievements | null;
  hunters: HunterStats | null;
  extras: Extras;
  /** Bytes on disk, 0 if not installed. */
  installed_bytes: number;
  /** Roughly what installing it downloads, from Steam's own app data. 0 if unknown. */
  download_bytes: number;
  /** Lent by someone in the user's Steam Family. */
  family: boolean;
  /** Has screenshots, taken on this computer or uploaded to the profile. */
  screenshots: boolean;
  store: StoreInfo | null;
  /** ProtonDB tier: platinum, gold, silver, bronze or borked. */
  proton: string | null;
  cover: string | null;
  collections: string[];
}

export interface Collection {
  id: string;
  name: string;
  count: number;
  dynamic: boolean;
  color: string | null;
}

/** Unsaved work: edits live in the app until they are saved to Steam. */
export interface DraftState {
  pending: number;
  changed: string[];
  undo: string | null;
  redo: string | null;
  steam_running: boolean;
  rebased: boolean;
}

export interface Library {
  games: Game[];
  collections: Collection[];
  tags: Record<string, string>;
  flags: { id: number; name: string }[];
  draft: DraftState;
  online: OnlineState;
  /** The user keeps their own collection order, with numbers in front of the names in Steam. */
  ordered: boolean;
}

/** What an edit changed: the collections, the draft, and the games that moved. */
export interface Patch {
  collections: Collection[];
  draft: DraftState;
  membership: Record<string, string[]>;
  value: string | null;
}

export interface Status {
  steam_found: boolean;
  steam_running: boolean;
  legacy_debug_port: boolean;
}

/** What a collection automatically collects. Mirrors the filters in the toolbar. */
export interface SmartRule {
  played: 'never' | 'played' | 'idle' | null;
  achievements: 'complete' | 'incomplete' | null;
  min_review: number;
  flags: number[];
  tags: number[];
  /** Further conditions with AND, OR and NOT. */
  condition?: Cond | null;
  /** Also take games out when they stop matching. */
  sync?: boolean;
}

/** A ready-made set of collections that fill themselves. */
export interface Preset {
  id: string;
  name: string;
  description: string;
  collections: { name: string; rule: SmartRule }[];
}

export interface PresetResult {
  patch: Patch;
  created: string[];
  attached: string[];
  kept: string[];
}

export const setOrdering = (on: boolean) => call<Patch>('set_ordering', { on });
/** Puts the collections in this order, by id. */
export const setOrder = (ids: string[]) => call<Patch>('set_order', { ids });

export const presets = () => call<Preset[]>('presets');
export const applyPreset = (id: string) => call<PresetResult>('apply_preset', { id });

export type Automation = 'off' | 'notify' | 'auto';

export interface Settings {
  /** null until the user has chosen, so the app can ask once. */
  automation: Automation | null;
  /** Online extras on or off. null until chosen. */
  online_stats: boolean | null;
  /** Two-letter country for prices; null follows this computer's region. */
  region: string | null;
  online_achievements: boolean;
  online_prices: boolean;
  online_protondb: boolean;
  online_hunters: boolean;
  reopen_steam: boolean;
  /** Look for a new version when the app starts. */
  check_updates: boolean;
  /** Numbered releases, or the newest build of the main branch. */
  update_channel: 'stable' | 'nightly';
  /** The Steam account to show; null follows whoever is signed in to Steam. */
  account: number | null;
}

/** A Steam account used on this computer. */
export interface AccountView {
  id: number;
  name: string;
  login: string;
  /** Steam's cached profile picture. */
  avatar: string | null;
  /** Changes made for this account that aren't in Steam yet. */
  pending: number;
}

export interface Accounts {
  accounts: AccountView[];
  current: number;
  /** Null when following Steam's sign-in. */
  picked: number | null;
  signed_in: number | null;
}

/** A newer version, found by `checkUpdate`. */
export interface UpdateInfo {
  version: string;
  current: string;
  notes: string | null;
}

export interface UpdateProgress {
  downloaded: number;
  total: number | null;
}

/** Sent by the background watcher. */
export type OrganizerEvent =
  | { type: 'changed'; steam_running: boolean }
  | { type: 'pending'; ops: OpView[]; notes: string[] }
  | { type: 'organized'; applied: number; saved: boolean; snapshot: string | null }
  | { type: 'account'; account: number };


export interface OpView {
  kind: 'create' | 'add' | 'remove' | 'rename' | 'delete';
  collection: string;
  count: number;
  sample: string[];
}

export interface SnapshotInfo {
  id: string;
  taken_at: number;
  label: string;
  collections: number;
  /** Games filed in those collections (each counted once). */
  games?: number;
}

export interface SaveReport {
  snapshot: string | null;
  changes: number;
  verified: boolean;
}

/** True when running inside the desktop app; false in a plain browser (demo mode). */
export const isDesktop = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

export const coverUrl = (path: string | null): string | null => (path ? (isDesktop && !path.startsWith('http') ? convertFileSrc(path) : path) : null);

// The demo's invented library is only loaded in a plain browser, so the app doesn't carry it.
const mock = () => import('./mock');

const call = <T>(cmd: string, args?: Record<string, unknown>): Promise<T> => (isDesktop ? invoke<T>(cmd, args) : mock().then((m) => m.call<T>(cmd, args)));

async function listen<T>(event: string, handler: (payload: T) => void): Promise<() => void> {
  if (!isDesktop) return (await mock()).listen(event, handler);
  const { listen } = await import('@tauri-apps/api/event');
  return listen<T>(event, (e) => handler(e.payload));
}

/** The background watcher. Returns a function that stops listening. */
export const onOrganizer = (handler: (e: OrganizerEvent) => void) => listen('organizer', handler);
export const onOnline = (handler: (e: OnlineProgress) => void) => listen('online', handler);
/** A source finished and saved what it fetched, while others may still be running. */
export const onOnlineSaved = (handler: () => void) => listen('online-saved', handler);

export const status = () => call<Status>('status');
export const disableDebugPort = () => call<void>('disable_debug_port');
export const getSettings = () => call<Settings>('get_settings');
export const setSettings = (settings: Settings) => call<void>('set_settings', { settings });
/** Reads the library. `onlineOnly` after an online refresh: only the cached online data is
 *  read again, since Steam's own files haven't changed. */
export const loadLibrary = (onlineOnly = false) => call<Library>('load_library', { onlineOnly });

export const moveGames = (appids: number[], addTo: string[], removeFrom: string[], label: string) =>
  call<Patch>('move_games', { appids, addTo, removeFrom, label });
export const createCollection = (name: string, appids: number[]) => call<Patch>('create_collection', { name, appids });
export const renameCollection = (id: string, name: string) => call<Patch>('rename_collection', { id, name });
export const deleteCollection = (id: string) => call<Patch>('delete_collection', { id });
export const duplicateCollection = (id: string) => call<Patch>('duplicate_collection', { id });
export const mergeCollections = (sources: string[], into: string) => call<Patch>('merge_collections', { sources, into });
export const setColor = (id: string, color: string | null) => call<Patch>('set_color', { id, color });
export const undo = () => call<Patch>('undo');
export const redo = () => call<Patch>('redo');
export const discard = () => call<Patch>('discard');
export const pending = () => call<OpView[]>('pending');
export const save = (closeSteam: boolean) => call<SaveReport>('save', { closeSteam });

export const smartRules = () => call<Record<string, SmartRule>>('smart_rules');
export const createSmartCollection = (name: string, rule: SmartRule, auto: boolean) => call<Patch>('create_smart_collection', { name, rule, auto });
export const setSmartRule = (id: string, rule: SmartRule | null) => call<void>('set_smart_rule', { id, rule });
export const fillCollection = (id: string) => call<Patch>('fill_collection', { id });

export const history = () => call<SnapshotInfo[]>('history');
export const restoreSnapshot = (id: string) => call<Patch>('restore_snapshot', { id });
export const previewRules = () => call<{ ops: OpView[]; notes: string[] }>('preview_rules');
export const applyRules = () => call<Patch>('apply_rules');
/** Starts a background refresh of whatever online extras are due. False if one is running. */
/** What a refresh is about to fetch, worked out before it starts. */
export interface OnlinePlan {
  requests: number;
  /** A rough estimate. */
  seconds: number;
  games: number;
}

/** Starts a refresh; null if one is already running or online details are off. */
export const refreshOnline = (country: string, force = false) => call<OnlinePlan | null>('refresh_online', { country, force });
/** Stops the refresh after the request in flight; what was fetched is kept. */
export const stopOnline = () => call<void>('stop_online');

// ---- feedback and crash reports ----

/** A crash log waiting to be sent or dismissed. */
export interface Crash {
  id: string;
  at: number;
  summary: string;
  text: string;
}

export interface Report {
  kind: 'bug' | 'idea' | 'other' | 'crash';
  message: string;
  email: string | null;
  details: string | null;
}

/** Set when this window was opened to report a crash. */
export const crashMode = () => call<Crash | null>('crash_mode');
export const pendingCrashes = () => call<Crash[]>('pending_crashes');
export const settleCrash = (id: string) => call<void>('settle_crash', { id });
export const canSendReports = () => call<boolean>('can_send_reports');
/** The technical part of a report, exactly as it would be sent. */
export const reportDetails = (recentErrors: string[]) => call<string>('report_details', { recentErrors });
export const sendReport = (report: Report) => call<void>('send_report', { report });
export const reportText = (report: Report) => call<string>('report_text', { report });
/** From the crash window: opens Depiled normally and closes the reporter. */
export const startNormally = () => call<void>('start_normally');

/** Puts text on the clipboard. False if the system wouldn't allow it. */
export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    const area = Object.assign(document.createElement('textarea'), { value: text });
    document.body.append(area);
    area.select();
    const ok = document.execCommand('copy');
    area.remove();
    return ok;
  }
}

export const accounts = () => call<Accounts>('accounts');
/** Shows another account's library, or (null) whoever is signed in to Steam. */
export const setAccount = (account: number | null) => call<void>('set_account', { account });

/** Asks GitHub for a newer build on the channel. Null when this is the newest. */
export const checkUpdate = (nightly: boolean) => call<UpdateInfo | null>('check_update', { nightly });
/** Downloads and installs the update `checkUpdate` found, then restarts into it. */
export const installUpdate = () => call<void>('install_update');
export const onUpdateProgress = (handler: (p: UpdateProgress) => void) => listen('update-progress', handler);
export const appVersion = (): Promise<string> => (isDesktop ? import('@tauri-apps/api/app').then((m) => m.getVersion()) : Promise.resolve('0.2.6'));
