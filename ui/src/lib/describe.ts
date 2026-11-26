// Plain-English summaries of filters and auto-fill rules, for chips and the collection bar.
import { fieldById, OPS } from './criteria';
import type { Cond } from './criteria';
import type { SmartRule } from './api';
import { app } from './store.svelte';

const flagName = (id: number) => app.library?.flags.find((f) => f.id === id)?.name ?? String(id);

/** One condition, e.g. "Publisher contains Annapurna and (Price less than 30 or On sale is yes)". */
export function describeCond(c: Cond): string {
  if ('field' in c) {
    const f = fieldById(c.field);
    if (!f) return c.field;
    const op = OPS[f.type].find(([id]) => id === c.op)?.[1] ?? c.op;
    const label = (v: unknown) =>
      f.options?.find(([id]) => id === v)?.[1] ??
      (c.field === 'tags' ? app.tagNames[Number(v)] : c.field === 'features' ? flagName(Number(v)) : c.field === 'collections' ? app.collections.find((x) => x.id === v)?.name : undefined) ??
      String(v ?? '…');
    if (f.type === 'bool') return `${f.label}: ${c.value === false ? 'no' : 'yes'}`;
    const v = c.op === 'known' || c.op === 'unknown' ? '' : Array.isArray(c.value) ? c.value.map(label).join(c.op === 'between' ? ' to ' : ', ') : label(c.value);
    const unit = f.id === 'last_played_days' ? 'days ago' : f.unit;
    return [f.label, op, v, v ? unit : ''].filter(Boolean).join(' ');
  }
  if ('not' in c) return 'none of: ' + ('any' in c.not ? c.not.any.map(describeCond).join(', ') : describeCond(c.not));
  const children = 'all' in c ? c.all : c.any;
  return children.map((child) => ('field' in child || children.length === 1 ? describeCond(child) : '(' + describeCond(child) + ')')).join('all' in c ? ' and ' : ' or ');
}

const PLAYED = { never: 'never played', played: 'played', idle: 'not played in 7+ days' } as const;
const ACH = { complete: 'every achievement unlocked', incomplete: 'achievements in progress' } as const;

/** A whole auto-fill rule, e.g. "never played, reviews 80% or better, Price less than 20". */
export function describeRule(rule: SmartRule): string {
  const out: string[] = [];
  if (rule.played) out.push(PLAYED[rule.played]);
  if (rule.achievements) out.push(ACH[rule.achievements]);
  if (rule.min_review) out.push(`reviews ${rule.min_review}% or better`);
  for (const f of rule.flags) out.push(flagName(f));
  for (const t of rule.tags) out.push(app.tagNames[t] ?? String(t));
  if (rule.condition) {
    const text = describeCond(rule.condition);
    if (text) out.push(text);
  }
  return out.join(', ');
}
