import { invoke, convertFileSrc } from '@tauri-apps/api/core';
import * as mock from './mock';

export interface Game {
  appid: number;
  name: string;
  playtime_minutes: number;
  last_played: number;
  categories: number[];
  tags: number[];
  metacritic: number;
  review_percent: number;
  cover: string | null;
  collections: string[];
}

export interface Collection {
  id: string;
  name: string;
  count: number;
  dynamic: boolean;
}

export interface Library {
  games: Game[];
  collections: Collection[];
  tags: Record<string, string>;
  flags: { id: number; name: string }[];
}

export interface Status {
  steam_found: boolean;
  steam_running: boolean;
  ready: boolean;
  flag_file: boolean;
}

export interface Achievements {
  unlocked: number;
  total: number;
}

/** What a collection automatically collects. Mirrors the filters in the toolbar. */
export interface SmartRule {
  played: 'never' | 'played' | 'idle' | null;
  achievements: 'complete' | 'incomplete' | null;
  min_review: number;
  flags: number[];
  tags: number[];
}

export interface SmartResult {
  snapshot: string | null;
  added: number;
}

export type Automation = 'off' | 'notify' | 'auto';

export interface Settings {
  /** null until the user has chosen, so the app can ask once. */
  automation: Automation | null;
}

/** Sent by the background watcher. */
export type OrganizerEvent =
  | { type: 'pending'; ops: OpView[]; notes: string[] }
  | { type: 'organized'; applied: number; snapshot: string | null; verified: boolean };

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
}

export interface RestoreReport {
  changes: string[];
  safety_snapshot: string | null;
  verified: boolean;
}

/** True when running inside the desktop app; false in a plain browser (demo mode). */
export const isDesktop = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

export const coverUrl = (path: string | null): string | null =>
  path ? (isDesktop ? convertFileSrc(path) : path) : null;

const call = <T>(cmd: string, args?: Record<string, unknown>): Promise<T> =>
  isDesktop ? invoke<T>(cmd, args) : mock.call<T>(cmd, args);

export const getSettings = () => call<Settings>('get_settings');
export const setSettings = (settings: Settings) => call<void>('set_settings', { settings });

/** Listens for the background watcher. Returns a function that stops listening. */
export async function onOrganizer(handler: (e: OrganizerEvent) => void): Promise<() => void> {
  if (!isDesktop) return mock.onOrganizer(handler);
  const { listen } = await import('@tauri-apps/api/event');
  return listen<OrganizerEvent>('organizer', (e) => handler(e.payload));
}

export const status = () => call<Status>('status');
export const enableSetup = () => call<void>('enable_setup');
export const loadLibrary = () => call<Library>('load_library');
export const loadAchievements = () => call<Record<string, Achievements>>('load_achievements');

/** Resolves to the id of the undo checkpoint taken first, or null if nothing changed. */
export const moveGames = (appids: number[], addTo: string[], removeFrom: string[], label: string) =>
  call<string | null>('move_games', { appids, addTo, removeFrom, label });
export const createCollection = (name: string, appids: number[]) =>
  call<string | null>('create_collection', { name, appids });
export const renameCollection = (id: string, name: string) =>
  call<string | null>('rename_collection', { id, name });
export const deleteCollection = (id: string) => call<string | null>('delete_collection', { id });

export const smartRules = () => call<Record<string, SmartRule>>('smart_rules');
export const createSmartCollection = (name: string, rule: SmartRule, auto: boolean) =>
  call<SmartResult>('create_smart_collection', { name, rule, auto });
export const setSmartRule = (id: string, rule: SmartRule | null) => call<void>('set_smart_rule', { id, rule });
export const fillCollection = (id: string) => call<SmartResult>('fill_collection', { id });

export const history = () => call<SnapshotInfo[]>('history');
export const restoreSnapshot = (id: string) => call<RestoreReport>('restore_snapshot', { id });
export const previewRules = () => call<{ ops: OpView[]; notes: string[] }>('preview_rules');
export const applyRules = () =>
  call<{ snapshot: string | null; applied: number; verified: boolean }>('apply_rules');
