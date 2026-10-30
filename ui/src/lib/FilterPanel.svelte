<script lang="ts">
  import { app } from './store.svelte';
  import type { AchFilter, PlayedFilter } from './store.svelte';

  let tagQuery = $state('');

  const played: [PlayedFilter, string][] = [
    ['any', 'Any'],
    ['never', 'Never played'],
    ['played', 'Played'],
    ['idle', 'Not played in 7+ days'],
  ];
  const ach: [AchFilter, string][] = [
    ['any', 'Any'],
    ['complete', '100% complete'],
    ['incomplete', 'In progress'],
  ];

  const shownTags = $derived(
    app.tagChoices.filter((t) => t.name.toLowerCase().includes(tagQuery.trim().toLowerCase())).slice(0, tagQuery ? 60 : 24),
  );
  const pretty = (s: string) => s.replace(/\b\w/g, (c) => c.toUpperCase());

  const chip = (on: boolean) => `btn btn-xs h-7 rounded-full px-3 font-medium ${on ? 'btn-primary' : 'btn-soft'}`;
</script>

<div class="flex flex-col gap-5">
  <section>
    <h3 class="mb-2 text-xs font-bold uppercase tracking-wider text-muted">Play status</h3>
    <div class="flex flex-wrap gap-1.5">
      {#each played as [v, label]}
        <button class={chip(app.played === v)} onclick={() => (app.played = v)}>{label}</button>
      {/each}
    </div>
  </section>

  <section>
    <h3 class="mb-2 text-xs font-bold uppercase tracking-wider text-muted">Achievements</h3>
    <div class="flex flex-wrap gap-1.5">
      {#each ach as [v, label]}
        <button class={chip(app.ach === v)} disabled={!app.hasAchievements && v !== 'any'} onclick={() => (app.ach = v)}>{label}</button>
      {/each}
    </div>
    {#if !app.hasAchievements}<p class="mt-1.5 text-xs text-muted">Loading achievement progress from Steam…</p>{/if}
  </section>

  <section>
    <h3 class="mb-2 flex justify-between text-xs font-bold uppercase tracking-wider text-muted">
      <span>Minimum review score</span><span class="normal-case tracking-normal">{app.minReview ? app.minReview + '%' : 'Any'}</span>
    </h3>
    <input type="range" class="range range-xs range-primary w-full" min="0" max="95" step="5" bind:value={app.minReview} aria-label="Minimum review score" />
  </section>

  <section>
    <h3 class="mb-2 text-xs font-bold uppercase tracking-wider text-muted">Features</h3>
    <div class="flex flex-wrap gap-1.5">
      {#each app.library?.flags ?? [] as f}
        <button class={chip(app.flagFilter.includes(f.id))} onclick={() => app.toggleFlag(f.id)}>{pretty(f.name)}</button>
      {/each}
    </div>
  </section>

  <section>
    <h3 class="mb-2 text-xs font-bold uppercase tracking-wider text-muted">Genres and tags</h3>
    <input type="search" class="input input-sm w-full" placeholder="Find a genre or tag" bind:value={tagQuery} />
    <div class="mt-2 flex flex-wrap gap-1.5">
      {#each shownTags as t (t.id)}
        <button class={chip(app.tagFilter.includes(t.id))} onclick={() => app.toggleTag(t.id)}>
          {t.name} <span class="opacity-60">{t.count}</span>
        </button>
      {/each}
    </div>
  </section>
</div>
