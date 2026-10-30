<script lang="ts">
  import { app } from './store.svelte';
  import Icon from './Icon.svelte';

  const id = $derived(app.view !== 'all' && app.view !== 'none' ? app.view : null);
  const collection = $derived(id ? app.collections.find((c) => c.id === id) : undefined);
  const rule = $derived(id ? app.smartRules[id] : undefined);
  const fresh = $derived(id ? app.newMatches(id).length : 0);

  const playedLabels = { never: 'Never played', played: 'Played', idle: 'Not played in 7+ days' } as const;
  const achLabels = { complete: '100% complete', incomplete: 'Achievements in progress' } as const;
  const pretty = (s: string) => s.replace(/\b\w/g, (c) => c.toUpperCase());

  const parts = $derived.by(() => {
    if (!rule) return [] as string[];
    const out: string[] = [];
    if (rule.played) out.push(playedLabels[rule.played]);
    if (rule.achievements) out.push(achLabels[rule.achievements]);
    if (rule.min_review) out.push(`${rule.min_review}%+ reviews`);
    for (const f of rule.flags) out.push(pretty(app.library?.flags.find((x) => x.id === f)?.name ?? String(f)));
    for (const t of rule.tags) out.push(app.tagNames[t] ?? String(t));
    return out;
  });
</script>

{#if collection && !collection.dynamic}
  {#if rule}
    <div class="mx-5 mb-3 flex items-center gap-3 rounded-box border border-base-300 bg-base-100 py-2.5 pl-3.5 pr-3">
      <span class="grid size-8 shrink-0 place-items-center rounded-field bg-primary text-primary-content"><Icon name="wand" size={16} /></span>
      <div class="flex min-w-0 flex-1 flex-wrap items-center gap-2">
        <strong class="text-sm">Auto-fills with</strong>
        {#each parts as p}<span class="badge badge-soft badge-sm">{p}</span>{/each}
      </div>
      <button class="btn btn-sm btn-ghost" onclick={() => app.openRuleDialog(collection.id)}>Edit</button>
      <button class="btn btn-sm btn-primary" disabled={fresh === 0 || app.busy} onclick={() => app.fillCollection(collection.id)}>
        {fresh === 0 ? 'Up to date' : `Add ${fresh} matching ${fresh === 1 ? 'game' : 'games'}`}
      </button>
    </div>
  {:else}
    <div class="mx-5 mb-3 flex items-center justify-between rounded-box border border-dashed border-base-300 py-1.5 pl-3.5 pr-2">
      <span class="text-sm text-muted">This collection is filled by hand.</span>
      <button class="btn btn-sm btn-ghost" onclick={() => app.openRuleDialog(collection.id)}><Icon name="wand" size={14} /> Make it fill itself…</button>
    </div>
  {/if}
{/if}
