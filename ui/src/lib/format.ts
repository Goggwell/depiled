const DAY = 86400;

export function fmtHours(minutes: number): string {
  if (minutes <= 0) return 'Not played';
  const h = minutes / 60;
  return h < 1 ? `${minutes} min` : `${h < 10 ? h.toFixed(1) : Math.round(h).toLocaleString()} h`;
}

/** A duration, for completion times. */
export function fmtSpan(minutes: number): string {
  if (minutes <= 0) return 'Unknown';
  if (minutes < 60) return `${minutes} min`;
  const h = minutes / 60;
  return `${h < 10 ? h.toFixed(1) : Math.round(h).toLocaleString()} h`;
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

export const fmtYear = (ts: number) => (ts ? String(new Date(ts * 1000).getFullYear()) : 'Unknown');

export function fmtSize(bytes: number): string {
  const gb = bytes / 1024 ** 3;
  return gb >= 1 ? `${gb < 10 ? gb.toFixed(1) : Math.round(gb)} GB` : `${Math.max(1, Math.round(bytes / 1024 ** 2))} MB`;
}