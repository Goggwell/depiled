<script lang="ts">
  import { tick } from 'svelte';
  import { countTests } from './criteria';
  import { describeCond } from './describe';
  import { app, saveLayout } from './store.svelte';
  import FilterPanel from './FilterPanel.svelte';
  import Icon from './Icon.svelte';
  import SortMenu from './SortMenu.svelte';

  let open = $state(false);
  // Built the first time it opens, then only hidden, so reopening doesn't rebuild every tag.
  let built = $state(false);
  let trigger: HTMLButtonElement;
  let filterDialog = $state<HTMLDivElement>();
  let advancedPane = $state(false);

  async function openFilters(advanced = false) {
    advancedPane = advanced || countTests(app.advanced) > 0;
    built = true;
    open = true;
    await tick();
    filterDialog?.focus();
  }
  function closeFilters() {
    open = false;
    trigger?.focus();
  }

  const describe = describeCond;

  // What's in the search box. On a big library the grid follows after a short pause in typing,
  // so each keystroke doesn't filter and redraw thousands of games; small ones follow at once.
  let typed = $state(app.search);
  let typing: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    typed = app.search; // cleared or set elsewhere
  });
  function type(value: string) {
    typed = value;
    clearTimeout(typing);
    const big = (app.library?.games.length ?? 0) > 3000;
    if (!big || !value) app.search = value;
    else typing = setTimeout(() => (app.search = value), 150);
  }

  const deckNames = ['Unknown on Deck', 'Deck unsupported', 'Deck playable', 'Deck verified'];
  const platformNames = (bits: number) => [['Windows', 1], ['macOS', 2], ['Linux', 4]].filter(([, b]) => bits & (b as number)).map(([n]) => n).join(' and ');
  const choiceText = { installed: ['Installed', 'Not installed'], vr: ['VR', 'No VR'], earlyAccess: ['Early access', 'Not early access'], hasAchievements: ['Has achievements', 'No achievements'], family: ['Family games', 'Not family games'] } as const;

  const playedLabels = { never: 'Never played', played: 'Played', idle: 'Not played in 7+ days' } as const;
  const achLabels = { complete: 'Every achievement', incomplete: 'Achievements in progress' } as const;
  const flagName = (id: number) => app.library?.flags.find((f) => f.id === id)?.name ?? String(id);
  const pretty = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);

  function saveAsCollection() {
    open = false;
    app.openNewCollection();
  }

  const chip = 'tagbox pr-1.5';

  let keys = $state(false);
  const SHORTCUTS: [string, string][] = [
    ['Arrows', 'Move between games'],
    ['Ctrl+click', 'Pick several games'],
    ['Shift+click', 'Pick a run of games'],
    ['Ctrl+A', 'Pick everything shown'],
    ['Esc', 'Clear the selection'],
    ['Delete', 'Take picked games out of this collection'],
    ['Ctrl+Z', 'Undo'],
    ['Ctrl+Shift+Z', 'Redo'],
    ['Ctrl+S', 'Review and save to Steam'],
    ['Shift+drag', 'Move to a collection instead of adding'],
  ];
  const total = $derived((app.library?.games ?? []).filter((g) => app.inView(g)).length);
  const viewColor = $derived(app.isCollectionView ? app.collections.find((c) => c.id === app.view)?.color : null);

  const layouts: ['grid' | 'compact' | 'list', string, string][] = [
    ['grid', 'grid', 'Large covers'],
    ['compact', 'gridSmall', 'Small covers'],
    ['list', 'list', 'Table'],
  ];
</script>

<svelte:window onclick={() => ((open = false), (keys = false))} onkeydown={(e) => { if (open && e.key === 'Escape') closeFilters(); if (e.key === 'Escape') keys = false; }} />

<header class="flex flex-col gap-3 px-6 pb-3 pt-5">
  <div class="flex items-center gap-3">
    {#if viewColor}<span class="h-6 w-[5px] shrink-0 rounded-[1px]" style="background:{viewColor}" aria-hidden="true"></span>{/if}
    <h1 class="min-w-0 truncate font-display text-[28px] font-bold leading-tight">{app.viewTitle}</h1>
    {#if app.library}<span class="shrink-0 pt-1.5 text-sm tabular-nums text-muted">{app.visible.length.toLocaleString()}{app.visible.length !== total ? ` of ${total.toLocaleString()}` : ''}</span>{/if}
    <span class="flex-1"></span>
    <div class="flex items-center gap-0.5">
      <button class="btn btn-ghost btn-sm btn-square" disabled={!app.draft.undo || app.busy} onclick={() => app.undo()} title={app.draft.undo ? `Undo ${app.draft.undo} (Ctrl+Z)` : 'Nothing to undo'} aria-label="Undo"><Icon name="undo" size={17} /></button>
      <button class="btn btn-ghost btn-sm btn-square" disabled={!app.draft.redo || app.busy} onclick={() => app.redo()} title={app.draft.redo ? `Redo ${app.draft.redo} (Ctrl+Shift+Z)` : 'Nothing to redo'} aria-label="Redo"><Icon name="redo" size={17} /></button>
      <button class="btn btn-ghost btn-sm btn-square" onclick={() => (app.dialog = 'history')} title="Earlier versions" aria-label="History"><Icon name="history" size={17} /></button>
      <div class="relative">
        <button class="btn btn-ghost btn-sm btn-square" aria-expanded={keys} title="Keyboard shortcuts" aria-label="Keyboard shortcuts" onclick={(e) => { e.stopPropagation(); keys = !keys; }}><Icon name="keyboard" size={17} /></button>
        {#if keys}
          <div class="absolute right-0 top-10 z-40 w-72 rounded-box border border-base-300 bg-base-100 p-3 text-[13px] shadow-[0_8px_28px_rgb(15_20_30/0.2)]" role="dialog" aria-label="Keyboard shortcuts" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.key === 'Escape' && (keys = false)}>
            <p class="mb-2 font-semibold">Keyboard shortcuts</p>
            <dl class="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1.5">
              {#each SHORTCUTS as [k, what]}<dt><kbd class="kbd kbd-sm">{k}</kbd></dt><dd class="text-muted">{what}</dd>{/each}
            </dl>
          </div>
        {/if}
      </div>
      <button class="btn btn-ghost btn-sm btn-square" onclick={() => app.openFeedback()} title="Send feedback or report a problem" aria-label="Send feedback"><Icon name="feedback" size={17} /></button>
      <button class="btn btn-ghost btn-sm btn-square" onclick={() => (app.dialog = 'settings')} title="Settings" aria-label="Settings"><Icon name="gear" size={17} /></button>
      <button class="btn btn-primary btn-sm ml-2" onclick={() => (app.dialog = 'organize')} title="Sort games into collections by your rules, after showing you what would change"><Icon name="rules" size={16} /> Auto-organize</button>
    </div>
  </div>

  <div class="flex flex-wrap items-center gap-2">
    <label class="input input-sm h-8 min-w-36 flex-1 max-w-sm items-center gap-2 bg-base-100">
      <Icon name="search" size={15} />
      <input type="search" aria-label="Search library" class="min-w-0 grow" placeholder="Search names, tags or studios" value={typed} oninput={(e) => type(e.currentTarget.value)} />
    </label>

    <div class="relative">
      <button
        class="btn btn-sm h-8 border-base-300 bg-base-100 font-medium {app.activeFilterCount ? 'border-base-content/40' : ''}"
        bind:this={trigger}
        aria-haspopup="dialog" aria-expanded={open}
        onclick={(e) => {
          e.stopPropagation();
          if (open) closeFilters(); else openFilters();
        }}
      >
        <Icon name="filter" size={15} /> Filters
        {#if app.activeFilterCount}<span class="rounded bg-primary px-1.5 text-xs leading-5 text-primary-content tabular-nums">{app.activeFilterCount}</span>{/if}
      </button>

      {#if built}
        <div
          class="absolute left-1/2 top-10 z-30 flex max-h-[calc(100vh-180px)] {open ? '' : 'hidden'} w-[520px] max-w-[calc(100vw-2rem)] -translate-x-1/2 flex-col overflow-hidden rounded-box border border-base-300 bg-base-100 shadow-[0_12px_36px_rgb(15_20_30/0.22)]"
          bind:this={filterDialog}
          role="dialog"
          aria-label="Filters"
          tabindex="-1"
          onclick={(e) => e.stopPropagation()}
          onkeydown={() => {}}
        >
          <div class="overflow-y-auto p-4"><FilterPanel bind:advanced={advancedPane} /></div>
          <footer class="flex items-center gap-2 border-t border-base-300 p-3">
            <button class="btn btn-ghost btn-sm" onclick={() => app.clearFilters()} disabled={!app.activeFilterCount}>Clear</button>
            <span class="flex-1"></span>
            {#if app.activeFilterCount}<button class="btn btn-sm" onclick={saveAsCollection}>Save as collection…</button>{/if}
            <button class="btn btn-primary btn-sm" onclick={closeFilters}>Show {app.visible.length.toLocaleString()}</button>
          </footer>
        </div>
      {/if}
    </div>

    <SortMenu />

    <div class="seg ml-auto" role="group" aria-label="Show as">
      {#each layouts as [v, icon, label]}
        <button class="grid place-items-center !px-2" onclick={() => ((app.layout = v), saveLayout(v))} aria-label={label} aria-pressed={app.layout === v} title={label}><Icon name={icon} size={15} /></button>
      {/each}
    </div>
  </div>

  {#if app.activeFilterCount}
    <div class="flex flex-wrap items-center gap-1.5">
      {#if app.readyAdvanced}
        {@const summary = describe(app.readyAdvanced)}
        <span class="inline-flex h-7 max-w-full items-stretch rounded-field border border-base-300 bg-base-100 text-[13px]">
          <button class="min-w-0 max-w-72 truncate px-2 hover:underline" title={summary} onclick={(e) => { e.stopPropagation(); openFilters(true); }}>{summary}</button>
          <button class="px-1.5 hover:bg-base-200" aria-label="Clear advanced conditions" onclick={() => (app.advanced = { all: [] })}><Icon name="close" size={12} /></button>
        </span>
      {/if}
      {#if app.played !== 'any'}<button class={chip} onclick={() => (app.played = 'any')}>{playedLabels[app.played]} <Icon name="close" size={12} /></button>{/if}
      {#if app.ach !== 'any'}<button class={chip} onclick={() => (app.ach = 'any')}>{achLabels[app.ach]} <Icon name="close" size={12} /></button>{/if}
      {#if app.minReview}<button class={chip} onclick={() => (app.minReview = 0)}>Reviews {app.minReview}% or better <Icon name="close" size={12} /></button>{/if}
      {#if app.maxPerfectHours}<button class={chip} onclick={() => (app.maxPerfectHours = 0)}>100% in {app.maxPerfectHours} h or less <Icon name="close" size={12} /></button>{/if}
      {#each Object.entries(choiceText) as [key, [yes, no]]}
        {@const value = app[key as keyof typeof choiceText]}
        {#if value !== 'any'}<button class={chip} onclick={() => (app[key as keyof typeof choiceText] = 'any')}>{value === 'yes' ? yes : no} <Icon name="close" size={12} /></button>{/if}
      {/each}
      {#if app.deck.length}<button class={chip} onclick={() => (app.deck = [])}>{app.deck.map((d) => deckNames[d]).join(', ')} <Icon name="close" size={12} /></button>{/if}
      {#if app.platforms}<button class={chip} onclick={() => (app.platforms = 0)}>Runs on {platformNames(app.platforms)} <Icon name="close" size={12} /></button>{/if}
      {#if app.yearFrom || app.yearTo}<button class={chip} onclick={() => (app.yearFrom = app.yearTo = 0)}>Released {app.yearFrom && app.yearTo ? `${app.yearFrom}–${app.yearTo}` : app.yearFrom ? `${app.yearFrom} or later` : `${app.yearTo} or earlier`} <Icon name="close" size={12} /></button>{/if}
      {#if app.screenshots !== 'any'}<button class={chip} onclick={() => (app.screenshots = 'any')}>{app.screenshots === 'yes' ? 'Has screenshots' : 'No screenshots'} <Icon name="close" size={12} /></button>{/if}
      {#if app.onSale !== 'any'}<button class={chip} onclick={() => (app.onSale = 'any')}>{app.onSale === 'yes' ? 'On sale' : 'Not on sale'} <Icon name="close" size={12} /></button>{/if}
      {#if app.maxPrice}<button class={chip} onclick={() => (app.maxPrice = 0)}>Costs {app.maxPrice} or less <Icon name="close" size={12} /></button>{/if}
      {#if app.minReviews}<button class={chip} onclick={() => (app.minReviews = 0)}>{app.minReviews.toLocaleString()}+ reviews <Icon name="close" size={12} /></button>{/if}
      {#if app.proton.length}<button class={chip} onclick={() => (app.proton = [])}>ProtonDB {app.proton.join(', ')} <Icon name="close" size={12} /></button>{/if}
      {#if app.studio.trim()}<button class={chip} onclick={() => (app.studio = '')}>Studio: {app.studio.trim()} <Icon name="close" size={12} /></button>{/if}
      {#each app.flagFilter as f}<button class={chip} onclick={() => app.toggleFlag(f)}>{pretty(flagName(f))} <Icon name="close" size={12} /></button>{/each}
      {#each app.tagFilter as t}<button class={chip} onclick={() => app.toggleTag(t)}>{app.tagNames[t] ?? t} <Icon name="close" size={12} /></button>{/each}
      <button class="btn btn-ghost btn-xs h-7" onclick={() => app.clearFilters()}>Clear all</button>
      <button class="btn btn-ghost btn-xs h-7" onclick={saveAsCollection}><Icon name="plus" size={13} /> Save as collection</button>
    </div>
  {/if}
</header>
