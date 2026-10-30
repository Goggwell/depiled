<script lang="ts">
  import { coverUrl } from './api';
  import type { Game } from './api';
  import { app, fmtAgo, fmtHours } from './store.svelte';
  import Icon from './Icon.svelte';

  let broken = $state<Record<number, boolean>>({});

  // ---- windowing ---------------------------------------------------------------------
  // Only the rows that are on screen (plus a few either side) are drawn. A library of tens of
  // thousands of games therefore costs the same to display as one of a few hundred. Rows have a
  // fixed height so every row's position can be worked out without measuring it.
  const PAD_X = 40; // the scroller's left and right padding
  const GAP_X = 16;
  const GAP_Y = 20;
  const MIN_CARD = 148;
  const META_H = 56; // title (two lines) and playtime under the cover
  const META_GAP = 8;
  const LIST_ROW = 52;
  const LIST_GAP = 2;
  const OVERSCAN = 3;

  let clientWidth = $state(900);
  let clientHeight = $state(600);
  let scrollTop = $state(0);
  let scroller = $state<HTMLDivElement>();

  const innerWidth = $derived(Math.max(clientWidth - PAD_X, MIN_CARD));
  const columns = $derived(app.layout === 'grid' ? Math.max(1, Math.floor((innerWidth + GAP_X) / (MIN_CARD + GAP_X))) : 1);
  const cardWidth = $derived((innerWidth - GAP_X * (columns - 1)) / columns);
  const rowHeight = $derived(app.layout === 'grid' ? cardWidth * 1.5 + META_GAP + META_H + GAP_Y : LIST_ROW + LIST_GAP);
  const rowCount = $derived(Math.ceil(app.visible.length / columns));
  const firstRow = $derived(Math.max(0, Math.floor(scrollTop / rowHeight) - OVERSCAN));
  const lastRow = $derived(Math.min(rowCount, Math.ceil((scrollTop + clientHeight) / rowHeight) + OVERSCAN));
  const shown = $derived(app.visible.slice(firstRow * columns, lastRow * columns));

  // Start at the top whenever what is being shown changes.
  $effect(() => {
    void [app.view, app.search, app.layout, app.sort];
    scroller?.scrollTo({ top: 0 });
    scrollTop = 0;
  });

  // ---- helpers -----------------------------------------------------------------------

  const hue = (id: number) => (id * 47) % 360;
  const initials = (name: string) =>
    name
      .split(/\s+/)
      .filter((w) => /\p{L}|\p{N}/u.test(w))
      .slice(0, 2)
      .map((w) => [...w][0])
      .join('')
      .toUpperCase() || '?';

  function dragStart(e: DragEvent, g: Game) {
    if (!app.selectedSet.has(g.appid)) app.selected = [g.appid];
    e.dataTransfer!.setData('application/x-games', JSON.stringify(app.selected));
    e.dataTransfer!.effectAllowed = 'copyMove';
    const n = app.selected.length;
    if (n > 1) {
      const ghost = document.createElement('div');
      ghost.textContent = `${n} games`;
      ghost.style.cssText = 'position:fixed;top:-100px;padding:8px 14px;border-radius:10px;background:#c4e456;color:#3c402b;font:700 14px system-ui';
      document.body.appendChild(ghost);
      e.dataTransfer!.setDragImage(ghost, 20, 20);
      setTimeout(() => ghost.remove());
    }
  }

  function done(g: Game) {
    const a = app.achievements[g.appid];
    return !!a && a.total > 0 && a.unlocked === a.total;
  }

  function onKey(e: KeyboardEvent) {
    const mod = e.ctrlKey || e.metaKey;
    const typing = e.target instanceof HTMLInputElement || e.target instanceof HTMLSelectElement;
    if (mod && e.key.toLowerCase() === 'a' && !typing) {
      e.preventDefault();
      app.selectAllVisible();
    } else if (e.key === 'Escape') {
      app.clearSelection();
    } else if (mod && e.key.toLowerCase() === 'z' && !typing) {
      e.preventDefault();
      app.undo();
    } else if (e.key === 'Delete' && !typing && app.selected.length && app.view !== 'all' && app.view !== 'none') {
      app.removeFromCollection(app.selected, app.view);
    }
  }
</script>

<svelte:window onkeydown={onKey} />

{#snippet art(g: Game)}
  {@const src = coverUrl(g.cover)}
  {#if src && !broken[g.appid]}
    <img {src} alt="" loading="lazy" draggable="false" class="size-full object-cover" onerror={() => (broken[g.appid] = true)} />
  {:else}
    <div class="grid size-full place-items-center text-3xl font-bold tracking-wide text-white/85" style="background:linear-gradient(160deg,hsl({hue(g.appid)} 10% 36%),hsl({hue(g.appid) + 40} 10% 22%))">
      {initials(g.name)}
    </div>
  {/if}
{/snippet}

<div
  bind:this={scroller}
  bind:clientWidth
  bind:clientHeight
  class="min-h-0 flex-1 overflow-y-auto px-5 pb-6"
  role="presentation"
  onscroll={() => (scrollTop = scroller?.scrollTop ?? 0)}
  onclick={(e) => e.target === e.currentTarget && app.clearSelection()}
>
  {#if app.loading && !app.library}
    <div class="grid h-full place-items-center text-muted"><span class="loading loading-ring loading-lg"></span></div>
  {:else if app.visible.length === 0}
    <div class="flex flex-col items-center gap-2 px-5 py-20 text-center">
      <strong class="text-lg font-bold">No games match</strong>
      <span class="text-muted">
        {app.search || app.activeFilterCount ? 'Try clearing the search or filters.' : 'This collection is empty. Drag games here to add them.'}
      </span>
      {#if app.search || app.activeFilterCount}
        <button class="btn btn-sm btn-soft mt-1" onclick={() => { app.search = ''; app.clearFilters(); }}>Clear search and filters</button>
      {/if}
    </div>
  {:else}
    <div style="padding-top:{firstRow * rowHeight}px;padding-bottom:{Math.max(0, rowCount - lastRow) * rowHeight}px">
      {#if app.layout === 'grid'}
        <div class="grid" style="grid-template-columns:repeat({columns},minmax(0,1fr));column-gap:{GAP_X}px;row-gap:{GAP_Y}px">
          {#each shown as g (g.appid)}
            <button
              class="group flex flex-col text-left"
              style="gap:{META_GAP}px"
              draggable="true"
              ondragstart={(e) => dragStart(e, g)}
              onclick={(e) => app.select(g, e)}
              title={g.name}
            >
              <div
                class="relative aspect-[2/3] w-full overflow-hidden rounded-box bg-base-300 outline-3 outline-offset-2 transition duration-150 group-hover:-translate-y-0.5 group-hover:shadow-xl {app.selectedSet.has(g.appid) ? 'outline-secondary' : 'outline-transparent'}"
              >
                {@render art(g)}
                {#if done(g)}
                  <span class="absolute right-2 top-2 grid size-7 place-items-center rounded-full bg-primary text-primary-content" title="All achievements unlocked"><Icon name="trophy" size={15} weight="fill" /></span>
                {/if}
              </div>
              <div class="overflow-hidden px-0.5" style="height:{META_H}px">
                <div class="line-clamp-2 text-sm font-semibold leading-tight">{g.name}</div>
                <div class="mt-0.5 text-xs tabular-nums text-muted">{fmtHours(g.playtime_minutes)}</div>
              </div>
            </button>
          {/each}
        </div>
      {:else}
        <div class="flex flex-col" style="gap:{LIST_GAP}px">
          {#each shown as g (g.appid)}
            <button
              class="flex items-center gap-3.5 rounded-field px-2 pr-3 text-left hover:bg-base-300/50 {app.selectedSet.has(g.appid) ? 'bg-base-300/70 outline-2 -outline-offset-2 outline-secondary' : ''}"
              style="height:{LIST_ROW}px"
              draggable="true"
              ondragstart={(e) => dragStart(e, g)}
              onclick={(e) => app.select(g, e)}
            >
              <div class="h-10 w-7 shrink-0 overflow-hidden rounded bg-base-300">{@render art(g)}</div>
              <span class="min-w-0 flex-1 truncate font-semibold">{g.name}</span>
              {#if done(g)}<span class="text-warning" title="All achievements unlocked"><Icon name="trophy" size={14} weight="fill" /></span>{/if}
              <span class="w-24 text-right text-sm tabular-nums text-muted">{fmtHours(g.playtime_minutes)}</span>
              <span class="w-24 text-right text-sm tabular-nums text-muted">{fmtAgo(g.last_played)}</span>
              <span class="w-14 text-right text-sm tabular-nums text-muted">{g.review_percent ? g.review_percent + '%' : '–'}</span>
            </button>
          {/each}
        </div>
      {/if}
    </div>
  {/if}
</div>
