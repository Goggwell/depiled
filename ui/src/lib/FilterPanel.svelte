<script lang="ts">
  import { untrack } from 'svelte';
  import { countTests } from './criteria';
  import { app } from './store.svelte';
  import RuleBuilder from './RuleBuilder.svelte';
  import Icon from './Icon.svelte';
  import type { AchFilter, Choice, PlayedFilter } from './store.svelte';

  // Which pane shows. The toolbar's popover stays mounted between openings and picks the pane
  // each time it opens; elsewhere it's picked once. Editing a condition keeps the pane open.
  let { advanced = $bindable(untrack(() => countTests(app.advanced) > 0)) }: { advanced?: boolean } = $props();

  let tagQuery = $state('');

  const played: [PlayedFilter, string][] = [
    ['any', 'Any'],
    ['never', 'Never played'],
    ['played', 'Played'],
    ['idle', 'Not in 7+ days'],
  ];
  const ach: [AchFilter, string][] = [
    ['any', 'Any'],
    ['complete', 'All unlocked'],
    ['incomplete', 'In progress'],
  ];

  // Steam lists some things both as a feature and as a player tag ("Co-op", "Single-player").
  // Each is offered once, as a feature; "Steam achievements" is covered by "Has achievements".
  const ACHIEVEMENTS_FEATURE = 22;
  const squash = (s: string) => s.toLowerCase().replace(/[^a-z0-9]/g, '');
  const features = $derived((app.library?.flags ?? []).filter((f) => f.id !== ACHIEVEMENTS_FEATURE));
  const featureNames = $derived(new Set(features.map((f) => squash(f.name))));
  const tags = $derived(app.tagChoices.filter((t) => !featureNames.has(squash(t.name))));
  const shownTags = $derived(tags.filter((t) => t.name.toLowerCase().includes(tagQuery.trim().toLowerCase())).slice(0, tagQuery ? 60 : 24));

  const pretty = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);
  const hasFamily = $derived((app.library?.games ?? []).some((g) => g.family));
  const choices: [Choice, string][] = [['any', 'Any'], ['yes', 'Yes'], ['no', 'No']];
  const deckLevels: [number, string][] = [[3, 'Verified'], [2, 'Playable'], [1, 'Unsupported'], [0, 'Unknown']];
  const platformBits: [number, string][] = [[1, 'Windows'], [2, 'macOS'], [4, 'Linux']];
  const toggleDeck = (d: number) => (app.deck = app.deck.includes(d) ? app.deck.filter((x) => x !== d) : [...app.deck, d]);
  const years = (e: Event) => Number((e.currentTarget as HTMLInputElement).value) || 0;
  const hasStore = $derived((app.library?.games ?? []).some((g) => g.store));
  const hasProton = $derived((app.library?.games ?? []).some((g) => g.proton));
  const tiers: [string, string][] = [['platinum', 'Platinum'], ['gold', 'Gold'], ['silver', 'Silver'], ['bronze', 'Bronze'], ['borked', 'Doesn’t run'], ['pending', 'Too few reports'], ['none', 'No reports']];
  const toggleTier = (t: string) => (app.proton = app.proton.includes(t) ? app.proton.filter((x) => x !== t) : [...app.proton, t]);

  /** Filters below "More filters" that are set, so the section opens itself when one is. */
  const moreSet = $derived(
    app.minReview > 0 || app.maxPerfectHours > 0 || app.hasAchievements !== 'any' || app.vr !== 'any' || app.earlyAccess !== 'any' || app.family !== 'any' ||
      app.maxPrice > 0 || app.minReviews > 0 || app.proton.length > 0 || app.deck.length > 0 || app.platforms > 0 || app.yearFrom > 0 || app.yearTo > 0 ||
      app.studio.trim() !== '' || app.flagFilter.length > 0 || app.tagFilter.length > 0,
  );
  let more = $state(untrack(() => moreSet));
  const quickCount = $derived(app.activeFilterCount - app.advancedCount);
</script>

{#snippet seg(label: string, value: string, options: [string, string][], set: (v: never) => void)}
  <div class="flex items-center justify-between gap-3 text-[13px]">
    <span>{label}</span>
    <div class="seg" role="group" aria-label={label}>
      {#each options as [v, text]}<button aria-pressed={value === v} onclick={() => set(v as never)}>{text}</button>{/each}
    </div>
  </div>
{/snippet}

<div class="flex flex-col gap-4">
  <div class="seg self-start" role="group" aria-label="Kind of filters">
    <button aria-pressed={!advanced} onclick={() => (advanced = false)}>Quick filters{quickCount ? ` (${quickCount})` : ''}</button>
    <button aria-pressed={advanced} onclick={() => (advanced = true)}>Advanced{app.advancedCount ? ` (${app.advancedCount})` : ''}</button>
  </div>

  {#if advanced}
    <section>
      <p class="mb-3 text-[13px] text-muted">Combine conditions with and, or and not. Groups can hold their own conditions.</p>
      {#if quickCount}
        <p class="mb-3 flex items-center gap-2 rounded-[4px] bg-base-200 px-2.5 py-2 text-[13px]">
          <span class="flex-1">{quickCount} quick {quickCount === 1 ? 'filter also applies' : 'filters also apply'}, on top of these.</span>
          <button class="btn btn-ghost btn-xs" onclick={() => (advanced = false)}>Show</button>
        </p>
      {/if}
      <RuleBuilder cond={app.advanced} />
      {#if app.unfinishedAdvanced}<p class="mt-2 text-xs text-muted" role="status">Conditions without a value are ignored until you fill them in.</p>{/if}
    </section>
  {:else}
    <section class="flex flex-col gap-2.5">
      {@render seg('Played', app.played, played, (v) => (app.played = v))}
      {@render seg('Achievements', app.ach, ach, (v) => (app.ach = v))}
      {@render seg('Installed', app.installed, choices, (v) => (app.installed = v))}
      {#if hasStore}{@render seg(`On sale (${app.library?.online.country})`, app.onSale, choices, (v) => (app.onSale = v))}{/if}
      <p class="text-xs text-muted">Achievement progress is known for {app.achievementsKnown.toLocaleString()} of your games. The rest count as unknown.</p>
    </section>

    <button class="flex items-center gap-1.5 self-start text-[13px] font-semibold" aria-expanded={more} onclick={() => (more = !more)}>
      <span class="transition-transform {more ? '' : '-rotate-90'}"><Icon name="caret" size={13} /></span>
      More filters
    </button>

    {#if more}
      <section class="flex flex-col gap-2.5">
        <h3 class="text-[13px] font-semibold">Scores and time</h3>
        <label class="flex flex-col gap-1.5 text-[13px]">
          <span class="flex justify-between"><span>Review score</span><span class="tabular-nums text-muted">{app.minReview ? `${app.minReview}% or better` : 'Any'}</span></span>
          <input type="range" class="range range-xs w-full" min="0" max="95" step="5" bind:value={app.minReview} />
        </label>
        <label class="flex flex-col gap-1.5 text-[13px]">
          <span class="flex justify-between"><span>Time to unlock everything</span><span class="tabular-nums text-muted">{app.maxPerfectHours ? `${app.maxPerfectHours} h or less` : 'Any'}</span></span>
          <input type="range" class="range range-xs w-full" min="0" max="200" step="5" bind:value={app.maxPerfectHours} disabled={app.huntersKnown === 0} />
          <span class="text-xs text-muted">{app.huntersKnown ? 'The median player’s time, from SteamHunters.' : 'Turn on completion times in Settings to use this.'}</span>
        </label>
      </section>

      <section class="flex flex-col gap-2.5">
        <h3 class="text-[13px] font-semibold">About the game</h3>
        {@render seg('Has achievements', app.hasAchievements, choices, (v) => (app.hasAchievements = v))}
        {@render seg('VR', app.vr, choices, (v) => (app.vr = v))}
        {@render seg('Early access', app.earlyAccess, choices, (v) => (app.earlyAccess = v))}
        {#if hasFamily}{@render seg('Family game', app.family, choices, (v) => (app.family = v))}{/if}
        <div class="grid grid-cols-2 gap-3">
          <label class="flex flex-col gap-1.5 text-[13px]">
            <span>Released</span>
            <span class="flex items-center gap-1.5">
              <input type="number" class="input input-sm w-20" placeholder="From" min="1980" max="2100" value={app.yearFrom || ''} onchange={(e) => (app.yearFrom = years(e))} aria-label="Released from year" />
              <span class="text-muted">to</span>
              <input type="number" class="input input-sm w-20" placeholder="To" min="1980" max="2100" value={app.yearTo || ''} onchange={(e) => (app.yearTo = years(e))} aria-label="Released up to year" />
            </span>
          </label>
          <label class="flex flex-col gap-1.5 text-[13px]">
            <span>Developer or publisher</span>
            <input type="search" class="input input-sm w-full" placeholder="For example Valve" bind:value={app.studio} />
          </label>
        </div>
      </section>

      {#if hasStore}
        <section class="flex flex-col gap-2.5 text-[13px]">
          <h3 class="font-semibold">In the store</h3>
          <label class="flex items-center justify-between gap-3">
            <span>Costs at most</span>
            <input type="number" class="input input-sm w-28" min="0" placeholder="Any price" value={app.maxPrice || ''} onchange={(e) => (app.maxPrice = Math.max(0, Number(e.currentTarget.value) || 0))} />
          </label>
          <label class="flex items-center justify-between gap-3">
            <span>At least this many reviews</span>
            <input type="number" class="input input-sm w-28" min="0" step="100" placeholder="Any" value={app.minReviews || ''} onchange={(e) => (app.minReviews = Math.max(0, Number(e.currentTarget.value) || 0))} />
          </label>
        </section>
      {/if}

      <section class="flex flex-col gap-2">
        <h3 class="text-[13px] font-semibold">Where it runs</h3>
        <div class="flex flex-wrap items-center gap-1.5" role="group" aria-label="Runs on">
          {#each platformBits as [bit, label]}<button class="tagbox" aria-pressed={(app.platforms & bit) !== 0} onclick={() => (app.platforms ^= bit)}>{label}</button>{/each}
        </div>
        <div class="flex flex-wrap items-center gap-1.5" role="group" aria-label="Steam Deck">
          <span class="mr-1 text-xs text-muted">Steam Deck</span>
          {#each deckLevels as [d, label]}<button class="tagbox" aria-pressed={app.deck.includes(d)} onclick={() => toggleDeck(d)}>{label}</button>{/each}
        </div>
        {#if hasProton}
          <div class="flex flex-wrap items-center gap-1.5" role="group" aria-label="Linux rating from ProtonDB">
            <span class="mr-1 text-xs text-muted" title="Player reports from ProtonDB on how well it runs on Linux and Steam Deck">Linux (ProtonDB)</span>
            {#each tiers as [t, label]}<button class="tagbox" aria-pressed={app.proton.includes(t)} onclick={() => toggleTier(t)}>{label}</button>{/each}
          </div>
        {/if}
      </section>

      <section class="flex flex-col gap-2">
        <h3 class="text-[13px] font-semibold">Features</h3>
        <div class="flex flex-wrap gap-1.5">
          {#each features as f (f.id)}
            <button class="tagbox" aria-pressed={app.flagFilter.includes(f.id)} onclick={() => app.toggleFlag(f.id)}>{pretty(f.name)}</button>
          {/each}
        </div>
      </section>

      <section class="flex flex-col gap-2">
        <h3 class="text-[13px] font-semibold">Genres and tags</h3>
        <input type="search" class="input input-sm w-full" aria-label="Find a genre or tag" placeholder="Find a genre or tag" bind:value={tagQuery} />
        <div class="flex flex-wrap gap-1.5">
          {#each shownTags as t (t.id)}
            <button class="tagbox" aria-pressed={app.tagFilter.includes(t.id)} onclick={() => app.toggleTag(t.id)}>
              {t.name} <span class="tabular-nums opacity-60">{t.count}</span>
            </button>
          {/each}
          {#if !shownTags.length}<p class="text-[13px] text-muted">No tag matches “{tagQuery.trim()}”.</p>{/if}
        </div>
      </section>
    {/if}
  {/if}
</div>
