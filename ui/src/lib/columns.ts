// Columns the list view can show, and how each one sorts. The name column is always there.
import type { Game } from './api';
import { fmtAgo, fmtHours, fmtSize, fmtSpan, fmtYear } from './format';

export type ColumnId =
  | 'kind'
  | 'played'
  | 'last'
  | 'reviews'
  | 'metacritic'
  | 'release'
  | 'developer'
  | 'perfect'
  | 'achievements'
  | 'deck'
  | 'size'
  | 'collections'
  | 'price'
  | 'discount'
  | 'reviewCount'
  | 'proton';

export type SortKey = 'name' | ColumnId;

export interface Column {
  id: ColumnId;
  label: string;
  /** Width in pixels; the name column takes the rest. */
  width: number;
  /** Higher-is-better columns sort descending first. */
  desc: boolean;
  text: (g: Game) => string;
  /** Sort value; `null` (unknown) always sorts last. */
  value: (g: Game) => number | string | null;
}

const KINDS: Record<string, string> = {
  game: 'Game',
  demo: 'Demo',
  playtest: 'Playtest',
  software: 'Software',
  tool: 'Tool',
  soundtrack: 'Soundtrack',
  video: 'Video',
  shortcut: 'Non-Steam',
  unknown: 'Unknown',
};
const PROTON = ['pending', 'borked', 'bronze', 'silver', 'gold', 'platinum'];
const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);
const DECK = ['–', 'Unsupported', 'Playable', 'Verified'];
const known = (n: number) => (n > 0 ? n : null);

export const COLUMNS: Column[] = [
  { id: 'kind', label: 'Type', width: 90, desc: false, text: (g) => (g.family ? 'Family game' : KINDS[g.kind]), value: (g) => (g.family ? 'Family game' : KINDS[g.kind]) },
  { id: 'played', label: 'Played', width: 84, desc: true, text: (g) => (g.kind === 'game' ? fmtHours(g.playtime_minutes) : '–'), value: (g) => g.playtime_minutes },
  { id: 'last', label: 'Last played', width: 104, desc: true, text: (g) => fmtAgo(g.last_played), value: (g) => g.last_played },
  { id: 'reviews', label: 'Reviews', width: 74, desc: true, text: (g) => (g.review_percent ? `${g.review_percent}%` : '–'), value: (g) => known(g.review_percent) },
  { id: 'metacritic', label: 'Metacritic', width: 84, desc: true, text: (g) => (g.metacritic ? String(g.metacritic) : '–'), value: (g) => known(g.metacritic) },
  { id: 'release', label: 'Released', width: 78, desc: true, text: (g) => (g.release ? fmtYear(g.release) : '–'), value: (g) => known(g.release) },
  { id: 'developer', label: 'Developer', width: 160, desc: false, text: (g) => g.developers[0] ?? '–', value: (g) => g.developers[0]?.toLowerCase() ?? null },
  { id: 'perfect', label: 'To 100%', width: 80, desc: false, text: (g) => (g.hunters?.median_minutes ? fmtSpan(g.hunters.median_minutes) : '–'), value: (g) => known(g.hunters?.median_minutes ?? 0) },
  {
    id: 'achievements',
    label: 'Achievements',
    width: 104,
    desc: true,
    text: (g) => (g.achievements?.total ? `${g.achievements.unlocked} / ${g.achievements.total}` : g.hunters?.achievements ? `– / ${g.hunters.achievements}` : '–'),
    value: (g) => (g.achievements?.total ? g.achievements.unlocked / g.achievements.total : null),
  },
  { id: 'deck', label: 'Steam Deck', width: 96, desc: true, text: (g) => DECK[g.extras.deck], value: (g) => known(g.extras.deck) },
  { id: 'size', label: 'Size', width: 78, desc: true, text: (g) => (g.installed_bytes ? fmtSize(g.installed_bytes) : '–'), value: (g) => known(g.installed_bytes) },
  { id: 'collections', label: 'Collections', width: 72, desc: true, text: (g) => String(g.collections.length), value: (g) => g.collections.length },
  { id: 'price', label: 'Price', width: 84, desc: false, text: (g) => g.store?.price_text ?? '–', value: (g) => g.store?.price_cents ?? null },
  { id: 'discount', label: 'Sale', width: 60, desc: true, text: (g) => (g.store?.discount ? `-${g.store.discount}%` : '–'), value: (g) => known(g.store?.discount ?? 0) },
  { id: 'reviewCount', label: 'Review count', width: 104, desc: true, text: (g) => (g.store?.review_count ? g.store.review_count.toLocaleString() : '–'), value: (g) => known(g.store?.review_count ?? 0) },
  { id: 'proton', label: 'ProtonDB', width: 84, desc: true, text: (g) => (g.proton ? cap(g.proton) : '–'), value: (g) => (g.proton ? PROTON.indexOf(g.proton) : null) },
];

export const DEFAULT_COLUMNS: ColumnId[] = ['played', 'last', 'reviews', 'perfect', 'deck'];

export const columnById = (id: ColumnId) => COLUMNS.find((c) => c.id === id)!;
