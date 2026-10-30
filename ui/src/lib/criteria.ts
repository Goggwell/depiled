import type { Achievements, Game, SmartRule } from './api';

export const IDLE_DAYS = 7;
const DAY = 86400;

export const emptyRule = (): SmartRule => ({ played: null, achievements: null, min_review: 0, flags: [], tags: [] });

export const ruleIsEmpty = (r: SmartRule) =>
  !r.played && !r.achievements && r.min_review === 0 && r.flags.length === 0 && r.tags.length === 0;

/** Same meaning as the Rust side: every condition that is set must hold. */
export function matchesRule(rule: SmartRule, g: Game, ach: Record<string, Achievements>, now = Date.now() / 1000): boolean {
  const started = g.playtime_minutes > 0 || g.last_played > 0;
  if (rule.played === 'never' && started) return false;
  if (rule.played === 'played' && !started) return false;
  if (rule.played === 'idle' && !(started && g.last_played > 0 && now - g.last_played >= IDLE_DAYS * DAY)) return false;
  if (rule.achievements) {
    const a = ach[g.appid];
    // Unknown progress, or a game with no achievements, is neither complete nor in progress.
    if (!a || a.total === 0 || (rule.achievements === 'complete') !== (a.unlocked === a.total)) return false;
  }
  if (g.review_percent < rule.min_review) return false;
  return rule.flags.every((f) => g.categories.includes(f)) && rule.tags.every((t) => g.tags.includes(t));
}
