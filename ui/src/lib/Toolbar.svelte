<script lang="ts">
  import { app } from './store.svelte';
  import type { Sort } from './store.svelte';
  import FilterPanel from './FilterPanel.svelte';
  import Icon from './Icon.svelte';

  let open = $state(false);

  const sorts: [Sort, string][] = [
    ['name', 'Name'],
    ['playtime', 'Most played'],
    ['recent', 'Recently played'],
    ['review', 'Best reviewed'],
  ];

  const playedLabels = { never: 'Never played', played: 'Played', idle: 'Not played in 7+ days' } as const;
  const achLabels = { complete: '100% complete', incomplete: 'In progress' } as const;
  const flagName = (id: number) => app.library?.flags.find((f) => f.id === id)?.name ?? String(id);
  const pretty = (s: string) => s.replace(/\b\w/g, (c) => c.toUpperCase());

  function saveAsCollection() {
    open = false;
    app.dialog = 'new';
  }
</script>

<svelte:window onclick={() => (open = false)} />

<header class="flex flex-col gap-3.5 px-5 pb-3.5 pt-5">
  <div class="flex items-center justify-between gap-3">
    <div class="flex min-w-0 items-baseline gap-3">
      <h1 class="truncate font-display text-[26px] font-semibold tracking-tight">{app.viewTitle}</h1>
      <span class="shrink-0 text-sm text-muted">{app.visible.length} {app.visible.length === 1 ? 'game' : 'games'}</span>
    </div>

    <div class="flex items-center gap-1">
      <button class="btn btn-ghost btn-sm" disabled={app.undoStack.length === 0 || app.busy} onclick={() => app.undo()} title="Undo the last change (Ctrl+Z)">
        <Icon name="undo" size={16} /> Undo
      </button>
      <button class="btn btn-ghost btn-sm" onclick={() => (app.dialog = 'history')} title="See and restore earlier versions">
        <Icon name="history" size={16} /> History
      </button>
      <button class="btn btn-ghost btn-sm btn-square" onclick={() => (app.dialog = 'settings')} aria-label="Settings" title="Settings">
        <Icon name="gear" size={18} />
      </button>
      <button class="btn btn-primary btn-sm ml-1" onclick={() => (app.dialog = 'organize')}>
        <Icon name="wand" size={16} /> Auto-organize
      </button>
    </div>
  </div>

  <div class="flex items-center gap-2.5">
    <label class="input input-sm h-9 w-full max-w-md items-center gap-2">
      <Icon name="search" size={16} />
      <input type="search" class="grow" placeholder="Search by name, genre or tag" bind:value={app.search} />
    </label>

    <div class="relative">
      <button
        class="btn btn-sm h-9 {app.activeFilterCount > 0 ? 'btn-primary btn-outline' : ''}"
        onclick={(e) => {
          e.stopPropagation();
          open = !open;
        }}
      >
        <Icon name="filter" size={16} /> Filters
        {#if app.activeFilterCount}<span class="badge badge-primary badge-sm">{app.activeFilterCount}</span>{/if}
      </button>

      {#if open}
        <div
          class="absolute left-0 top-11 z-30 flex max-h-[calc(100vh-180px)] w-[440px] flex-col overflow-hidden rounded-box border border-base-300 bg-base-100 shadow-2xl"
          role="dialog"
          aria-label="Filters"
          tabindex="-1"
          onclick={(e) => e.stopPropagation()}
          onkeydown={(e) => e.key === 'Escape' && (open = false)}
        >
          <div class="overflow-y-auto p-4"><FilterPanel /></div>
          <footer class="flex items-center gap-2 border-t border-base-300 bg-base-100 p-3">
            <button class="btn btn-ghost btn-sm" onclick={() => app.clearFilters()} disabled={!app.activeFilterCount}>Clear</button>
            <span class="flex-1"></span>
            {#if app.activeFilterCount}
              <button class="btn btn-soft btn-sm" onclick={saveAsCollection} title="Make a collection from these filters">Save as collection…</button>
            {/if}
            <button class="btn btn-primary btn-sm" onclick={() => (open = false)}>Show {app.visible.length} games</button>
          </footer>
        </div>
      {/if}
    </div>

    <select class="select select-sm h-9 w-44" bind:value={app.sort} aria-label="Sort by">
      {#each sorts as [v, label]}<option value={v}>{label}</option>{/each}
    </select>

    <div class="join ml-auto">
      <button class="btn join-item btn-sm h-9 btn-square {app.layout === 'grid' ? 'btn-active' : ''}" onclick={() => (app.layout = 'grid')} aria-label="Grid view" title="Grid"><Icon name="grid" size={16} /></button>
      <button class="btn join-item btn-sm h-9 btn-square {app.layout === 'list' ? 'btn-active' : ''}" onclick={() => (app.layout = 'list')} aria-label="List view" title="List"><Icon name="list" size={16} /></button>
    </div>
  </div>

  {#if app.activeFilterCount}
    <div class="flex flex-wrap items-center gap-1.5">
      {#if app.played !== 'any'}<button class="btn btn-primary btn-xs h-7 rounded-full" onclick={() => (app.played = 'any')}>{playedLabels[app.played]} <Icon name="close" size={12} /></button>{/if}
      {#if app.ach !== 'any'}<button class="btn btn-primary btn-xs h-7 rounded-full" onclick={() => (app.ach = 'any')}>{achLabels[app.ach]} <Icon name="close" size={12} /></button>{/if}
      {#if app.minReview}<button class="btn btn-primary btn-xs h-7 rounded-full" onclick={() => (app.minReview = 0)}>{app.minReview}%+ reviews <Icon name="close" size={12} /></button>{/if}
      {#each app.flagFilter as f}<button class="btn btn-primary btn-xs h-7 rounded-full" onclick={() => app.toggleFlag(f)}>{pretty(flagName(f))} <Icon name="close" size={12} /></button>{/each}
      {#each app.tagFilter as t}<button class="btn btn-primary btn-xs h-7 rounded-full" onclick={() => app.toggleTag(t)}>{app.tagNames[t] ?? t} <Icon name="close" size={12} /></button>{/each}
      <button class="btn btn-ghost btn-xs h-7" onclick={() => app.clearFilters()}>Clear all</button>
      <button class="btn btn-soft btn-xs h-7" onclick={saveAsCollection}><Icon name="plus" size={13} /> Save as collection</button>
    </div>
  {/if}
</header>
