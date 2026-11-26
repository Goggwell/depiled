<script lang="ts">
  import { tick } from 'svelte';
  import { coverUrl } from './api';
  import type { Game } from './api';
  import { app, fmtAgo, fmtHours, fmtSpan, NO_COLOR } from './store.svelte';
  import Icon from './Icon.svelte';
  import { COLUMNS, columnById } from './columns';
  import type { SortKey } from './columns';

  let broken = $state<Record<number, boolean>>({});
  let picking = $state(false);
  const cols = $derived(app.columns.map(columnById));

  // ---- windowing ---------------------------------------------------------------------
  // Only the rows on screen (plus a few either side) are drawn, so a library of tens of
  // thousands of games costs the same to display as a few hundred. Rows have a fixed height,
  // so every row's position is known without measuring it.
  const PAD_X = 48; // the scroller's left and right padding
  const LIST_ROW = 44;
  const OVERSCAN = 3;
  // Large covers, or small ones that fit about twice as many on screen.
  const SIZES = {
    grid: { min: 146, gapX: 18, gapY: 22, meta: 40, metaGap: 8 },
    compact: { min: 100, gapX: 12, gapY: 16, meta: 20, metaGap: 6 },
  };

  let clientWidth = $state(900);
  let clientHeight = $state(600);
  let scrollTop = $state(0);
  let scroller = $state<HTMLDivElement>();

  const covers = $derived(app.layout !== 'list');
  const size = $derived(SIZES[app.layout === 'compact' ? 'compact' : 'grid']);
  const innerWidth = $derived(Math.max(clientWidth - PAD_X, size.min));
  const columns = $derived(covers ? Math.max(1, Math.floor((innerWidth + size.gapX) / (size.min + size.gapX))) : 1);
  const cardWidth = $derived((innerWidth - size.gapX * (columns - 1)) / columns);
  const rowHeight = $derived(covers ? cardWidth * 1.5 + size.metaGap + size.meta + size.gapY : LIST_ROW);

  // ---- table columns ------------------------------------------------------------------
  // The name always keeps room to be read. Columns that don't fit are left off from the end
  // of the list (the order they were chosen in), and the header says how many.
  const NAME_MIN = 220;
  const CELL_GAP = 16;
  const fitted = $derived.by(() => {
    let room = innerWidth - 6 - 24 - CELL_GAP - 36 - NAME_MIN;
    const out = [];
    for (const c of cols) {
      if (room < c.width + CELL_GAP) break;
      out.push(c);
      room -= c.width + CELL_GAP;
    }
    return out;
  });
  const hiddenCols = $derived(cols.length - fitted.length);
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
      ghost.style.cssText = 'position:fixed;top:-100px;padding:6px 12px;border-radius:6px;background:#1a2232;color:#fff;font:600 13px "Bricolage Grotesque Variable",system-ui';
      document.body.appendChild(ghost);
      e.dataTransfer!.setDragImage(ghost, 20, 16);
      setTimeout(() => ghost.remove());
    }
  }

  const KIND_LABEL: Record<string, string> = { demo: 'Demo', playtest: 'Playtest', unknown: 'No details on this computer', software: 'Software', tool: 'Tool', soundtrack: 'Soundtrack', video: 'Video', shortcut: 'Added to Steam by hand' };
  const complete = (g: Game) => !!g.achievements && g.achievements.total > 0 && g.achievements.unlocked === g.achievements.total;
  // Only collections the user gave a colour; grey marks for the rest would be noise. Inside a
  // collection, its own mark would be on every game, so it is left off.
  const marks = (g: Game) => g.collections.filter((id) => id !== app.view).map((id) => app.colorOf.get(id)).filter((c): c is string => !!c && c !== NO_COLOR);

  /** Arrow keys move the selection: across and down the covers, or up and down the table. */
  async function move(key: string) {
    const list = app.visible;
    if (!list.length) return false;
    const steps: Record<string, number> = covers ? { ArrowLeft: -1, ArrowRight: 1, ArrowUp: -columns, ArrowDown: columns } : { ArrowUp: -1, ArrowDown: 1 };
    const step = steps[key];
    if (step === undefined) return false;
    const at = app.anchor === null ? -1 : list.findIndex((g) => g.appid === app.anchor);
    const next = at < 0 ? 0 : Math.min(list.length - 1, Math.max(0, at + step));
    app.select(list[next], {});
    const row = Math.floor(next / columns);
    const top = row * rowHeight;
    if (top < scrollTop) scroller?.scrollTo({ top });
    else if (top + rowHeight > scrollTop + clientHeight) scroller?.scrollTo({ top: top + rowHeight - clientHeight });
    await tick();
    scroller?.querySelector<HTMLElement>(`[data-appid="${list[next].appid}"]`)?.focus({ preventScroll: true });
    return true;
  }

  function onKey(e: KeyboardEvent) {
    const mod = e.ctrlKey || e.metaKey;
    const typing = e.target instanceof HTMLInputElement || e.target instanceof HTMLSelectElement || e.target instanceof HTMLTextAreaElement;
    if (typing || app.dialog || (e.target instanceof Element && e.target.closest('[role="dialog"],[role="menu"]'))) return;
    if (!mod && e.key.startsWith('Arrow') && !(e.target instanceof Element && e.target.closest('aside'))) {
      e.preventDefault();
      void move(e.key);
    } else if (mod && e.key.toLowerCase() === 'a') {
      e.preventDefault();
      app.selectAllVisible();
    } else if (e.key === 'Escape') {
      app.clearSelection();
    } else if (mod && e.key.toLowerCase() === 'z') {
      e.preventDefault();
      if (e.shiftKey) app.redo();
      else app.undo();
    } else if (mod && e.key.toLowerCase() === 'y') {
      e.preventDefault();
      app.redo();
    } else if (mod && e.key.toLowerCase() === 's') {
      e.preventDefault();
      if (app.draft.pending) app.dialog = 'save';
    } else if (e.key === 'Delete' && app.selected.length && app.isCollectionView) {
      void takeOut();
    }
  }

  /** Takes the picked games out of the open collection, then picks the game that took their
   *  place, so going through a collection with Delete and the arrow keys keeps its spot. */
  async function takeOut() {
    const list = app.visible;
    const gone = new Set(app.selected);
    const at = Math.max(0, list.findIndex((g) => gone.has(g.appid)));
    const next = list.slice(at).find((g) => !gone.has(g.appid)) ?? list.slice(0, at).reverse().find((g) => !gone.has(g.appid));
    await app.removeFromCollection(app.selected, app.view);
    if (!next) return app.clearSelection();
    app.select(next, {});
    await tick();
    scroller?.querySelector<HTMLElement>(`[data-appid="${next.appid}"]`)?.focus({ preventScroll: true });
  }
</script>

<svelte:window onkeydown={onKey} onclick={() => (picking = false)} />

{#snippet head(key: SortKey, label: string, width: number)}
  {@const on = app.sort === key}
  <button
    class="flex h-full items-center gap-1 hover:text-base-content {width ? 'shrink-0' : 'min-w-0 flex-1'} {on ? 'font-semibold text-base-content' : ''} {key === 'name' || key === 'developer' || key === 'kind' ? 'justify-start' : 'justify-end'}"
    style={width ? `width:${width}px` : ''}
    onclick={() => app.sortBy(key)}
    aria-sort={on ? (app.sortDesc ? 'descending' : 'ascending') : 'none'}
    role="columnheader"
  >
    {label}{#if on}<span class={app.sortDesc ? '' : 'rotate-180'}><Icon name="caret" size={11} /></span>{/if}
  </button>
{/snippet}

{#snippet art(g: Game)}
  {@const src = coverUrl(g.cover)}
  {#if src && !broken[g.appid]}
    <img {src} alt="" loading="lazy" draggable="false" class="size-full object-cover" onerror={() => (broken[g.appid] = true)} />
  {:else}
    <div class="flex size-full flex-col justify-end bg-base-300 p-2.5">
      <span class="font-display text-2xl font-bold leading-none text-base-content/40">{initials(g.name)}</span>
    </div>
  {/if}
{/snippet}

<div
  bind:this={scroller}
  bind:clientWidth
  bind:clientHeight
  class="min-h-0 flex-1 overflow-y-auto px-6 pb-6"
  role="presentation"
  onscroll={() => (scrollTop = scroller?.scrollTop ?? 0)}
  onclick={(e) => e.target === e.currentTarget && app.clearSelection()}
>
  {#if app.loading && !app.library}
    <div class="grid h-full place-items-center text-muted"><span class="loading loading-ring loading-lg"></span></div>
  {:else if app.visible.length === 0}
    <div class="flex max-w-md flex-col gap-2 py-16">
      <h2 class="font-display text-xl font-semibold">{app.search || app.activeFilterCount ? 'No games match' : 'Nothing here yet'}</h2>
      <p class="text-muted">
        {app.search || app.activeFilterCount ? 'Clear the search or a filter to see more.' : 'Drag games onto this collection in the sidebar to add them.'}
      </p>
      {#if app.search || app.activeFilterCount}
        <button class="btn btn-sm mt-2 w-fit" onclick={() => { app.search = ''; app.clearFilters(); }}>Clear search and filters</button>
      {/if}
    </div>
  {:else}
    <div style="padding-top:{firstRow * rowHeight}px;padding-bottom:{Math.max(0, rowCount - lastRow) * rowHeight}px">
      {#if covers}
        {@const small = app.layout === 'compact'}
        <div class="grid" style="grid-template-columns:repeat({columns},minmax(0,1fr));column-gap:{size.gapX}px;row-gap:{size.gapY}px">
          {#each shown as g (g.appid)}
            {@const on = app.selectedSet.has(g.appid)}
            <button class="group flex flex-col text-left focus-visible:outline-none" style="gap:{size.metaGap}px" data-appid={g.appid} draggable="true" ondragstart={(e) => dragStart(e, g)} onclick={(e) => app.select(g, e)} title={g.name} aria-pressed={on}>
              <div class="relative aspect-[2/3] w-full overflow-hidden rounded-[5px] bg-base-300 shadow-[0_1px_2px_rgb(15_20_30/0.18)] ring-offset-2 ring-offset-base-200 transition-shadow group-hover:shadow-[0_4px_12px_rgb(15_20_30/0.22)] group-focus-visible:outline group-focus-visible:outline-2 group-focus-visible:outline-offset-[5px] group-focus-visible:outline-base-content {on ? 'ring-[3px] ring-base-content' : ''}">
                {@render art(g)}
                {#if marks(g).length}
                  <!-- One coloured bar per collection the game is filed in. -->
                  <div class="absolute inset-x-0 bottom-0 flex h-1">
                    {#each marks(g) as color}<span class="flex-1" style="background:{color}"></span>{/each}
                  </div>
                {/if}
                {#if g.family}
                  <span class="absolute left-1.5 top-1.5 flex items-center gap-1 rounded-[4px] bg-info px-1.5 py-0.5 text-[11px] font-semibold text-info-content shadow-sm" title="Lent by someone in your Steam Family"><Icon name="family" size={12} weight="bold" />{#if !small} Family{/if}</span>
                {/if}
                {#if g.store?.discount}
                  <span class="absolute bottom-2 left-1.5 rounded-[4px] bg-success px-1.5 py-0.5 text-[11px] font-semibold text-success-content shadow-sm">-{g.store.discount}%</span>
                {/if}
                {#if complete(g)}
                  <span class="absolute right-1.5 top-1.5 grid size-6 place-items-center rounded-full bg-base-100/90 text-base-content shadow-sm" title="Every achievement unlocked"><Icon name="trophy" size={13} weight="fill" /></span>
                {/if}
              </div>
              <div class="overflow-hidden" style="height:{size.meta}px">
                <div class="truncate font-medium leading-snug {small ? 'text-xs' : 'text-[13px]'}">{g.name}</div>
                {#if !small}
                  {@const a = g.achievements}
                  <div class="flex items-center justify-between gap-2 text-xs tabular-nums text-muted">
                    <span class="truncate">{g.kind === 'game' ? fmtHours(g.playtime_minutes) : KIND_LABEL[g.kind]}</span>
                    {#if a && a.total > 0 && a.unlocked > 0 && a.unlocked < a.total}
                      <span class="flex shrink-0 items-center gap-0.5" title="{a.unlocked} of {a.total} achievements"><Icon name="trophy" size={11} />{a.unlocked}/{a.total}</span>
                    {/if}
                  </div>
                {/if}
              </div>
            </button>
          {/each}
        </div>
      {:else}
        <div class="flex flex-col" role="table" aria-label="Games">
          <div class="sticky top-0 z-10 flex h-8 items-center gap-4 border-b border-base-300 bg-base-200 pl-1.5 pr-1 text-xs text-muted" role="row">
            <span class="w-6 shrink-0"></span>
            {@render head('name', 'Name', 0)}
            {#each fitted as col (col.id)}{@render head(col.id, col.label, col.width)}{/each}
            <div class="relative">
              <button class="btn btn-ghost btn-xs h-6 gap-1 px-1 {hiddenCols ? 'text-base-content' : 'btn-square'}" aria-label={hiddenCols ? `Choose columns. ${hiddenCols} don't fit` : 'Choose columns'} title={hiddenCols ? `${hiddenCols} more ${hiddenCols === 1 ? 'column doesn’t' : 'columns don’t'} fit. Widen the window or choose fewer.` : 'Choose columns'} onclick={(e) => { e.stopPropagation(); picking = !picking; }}>
                {#if hiddenCols}<span class="tabular-nums">+{hiddenCols}</span>{/if}<Icon name="dots" size={16} weight="bold" />
              </button>
              {#if picking}
                <div class="absolute right-0 top-7 z-30 max-h-[min(60vh,440px)] w-56 overflow-y-auto rounded-box border border-base-300 bg-base-100 p-1.5 text-[13px] text-base-content shadow-[0_8px_28px_rgb(15_20_30/0.2)]" role="menu" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.key === 'Escape' && (picking = false)}>
                  <p class="px-2 pb-1 pt-0.5 text-xs text-muted">{hiddenCols ? `${hiddenCols} of these ${hiddenCols === 1 ? 'doesn’t' : 'don’t'} fit at this width.` : 'Columns'}</p>
                  {#each COLUMNS as col (col.id)}
                    <label class="flex h-7 cursor-pointer items-center gap-2.5 rounded-field px-2 hover:bg-base-200">
                      <input type="checkbox" class="checkbox checkbox-xs" checked={app.columns.includes(col.id)} onchange={() => app.toggleColumn(col.id)} />
                      {col.label}
                    </label>
                  {/each}
                </div>
              {/if}
            </div>
          </div>
          {#each shown as g (g.appid)}
            {@const on = app.selectedSet.has(g.appid)}
            <button
              class="flex items-center gap-4 border-b border-base-300/60 pl-1.5 pr-9 text-left text-[13px] focus-visible:-outline-offset-2 {on ? 'bg-base-100 font-medium outline outline-1 -outline-offset-1 outline-base-content/50' : 'hover:bg-base-100/60'}"
              style="height:{LIST_ROW}px"
              data-appid={g.appid}
              draggable="true"
              ondragstart={(e) => dragStart(e, g)}
              onclick={(e) => app.select(g, e)}
              aria-selected={on}
              role="row"
            >
              <div class="h-9 w-6 shrink-0 overflow-hidden rounded-[3px] bg-base-300" role="cell">{@render art(g)}</div>
              <span class="flex min-w-0 flex-1 items-center gap-2" role="cell">
                <span class="truncate font-medium">{g.name}</span>
                {#if g.family}<span class="flex shrink-0 items-center gap-1 rounded-[4px] bg-info px-1.5 text-[11px] font-semibold leading-5 text-info-content" title="Lent by someone in your Steam Family"><Icon name="family" size={11} weight="bold" /> Family</span>{/if}
                {#if complete(g)}<span class="shrink-0 text-muted" title="Every achievement unlocked"><Icon name="trophy" size={13} weight="fill" /></span>{/if}
                <span class="flex shrink-0 gap-1">{#each marks(g) as color}<span class="spine" style="background:{color};height:12px"></span>{/each}</span>
              </span>
              {#each fitted as col (col.id)}
                <span class="shrink-0 truncate font-normal tabular-nums text-muted {col.id === 'developer' || col.id === 'kind' ? 'text-left' : 'text-right'}" style="width:{col.width}px" role="cell">{col.text(g)}</span>
              {/each}
            </button>
          {/each}
        </div>      {/if}
    </div>
  {/if}
</div>
