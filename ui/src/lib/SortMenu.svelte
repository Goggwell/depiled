<script lang="ts">
  // One control for sorting: what to sort by and which way, said in words ("Last played,
  // newest first"). In the table it offers the columns that are showing; with covers, all of
  // them.
  import { tick } from 'svelte';
  import { app } from './store.svelte';
  import { COLUMNS, columnById } from './columns';
  import type { SortKey } from './columns';
  import Icon from './Icon.svelte';

  let open = $state(false);
  let button = $state<HTMLButtonElement>();
  let menu = $state<HTMLDivElement>();

  const label = (k: SortKey) => (k === 'name' ? 'Name' : columnById(k).label);

  /** How each direction reads for this kind of value: [ascending, descending]. */
  function directions(k: SortKey): [string, string] {
    switch (k) {
      case 'name':
      case 'kind':
      case 'developer':
        return ['A to Z', 'Z to A'];
      case 'last':
      case 'release':
        return ['Oldest first', 'Newest first'];
      case 'price':
        return ['Cheapest first', 'Most expensive first'];
      case 'size':
        return ['Smallest first', 'Largest first'];
      case 'perfect':
        return ['Shortest first', 'Longest first'];
      case 'played':
        return ['Least played first', 'Most played first'];
      default:
        return ['Lowest first', 'Highest first'];
    }
  }

  const keys = $derived.by(() => {
    const shown: SortKey[] = ['name', ...(app.layout === 'list' ? app.columns : COLUMNS.map((c) => c.id))];
    // Sorting by a column that was then hidden: keep it listed so the button isn't a mystery.
    return shown.includes(app.sort) ? shown : [...shown, app.sort];
  });
  const [asc, desc] = $derived(directions(app.sort));
  // "Last played, newest first"; "A to Z" keeps its capitals.
  const lower = (s: string) => (/^[A-Z] to [A-Z]$/.test(s) ? s : s.charAt(0).toLowerCase() + s.slice(1));
  const summary = $derived(`${label(app.sort)}, ${lower(app.sortDesc ? desc : asc)}`);

  async function toggle() {
    open = !open;
    if (open) {
      await tick();
      menu?.querySelector<HTMLElement>('[aria-checked="true"]')?.focus();
    }
  }
  function close() {
    open = false;
    button?.focus();
  }
  function keydown(e: KeyboardEvent) {
    const items = [...(menu?.querySelectorAll<HTMLElement>('[role="menuitemradio"]') ?? [])];
    const at = items.indexOf(document.activeElement as HTMLElement);
    const go = (i: number) => items[(i + items.length) % items.length]?.focus();
    if (e.key === 'ArrowDown') go(at + 1);
    else if (e.key === 'ArrowUp') go(at - 1);
    else if (e.key === 'Escape' || e.key === 'Tab') close();
    else return;
    e.preventDefault();
    e.stopPropagation();
  }
  const item = 'flex w-full items-center gap-2 rounded-[4px] px-2 py-1.5 text-left hover:bg-base-200 focus-visible:bg-base-200 focus-visible:outline-none';
</script>

<svelte:window onclick={() => (open = false)} />

<div class="relative">
  <button
    bind:this={button}
    class="btn btn-sm h-8 max-w-64 gap-1.5 border-base-300 bg-base-100 font-normal"
    aria-haspopup="menu"
    aria-expanded={open}
    title="Sort"
    onclick={(e) => {
      e.stopPropagation();
      toggle();
    }}
  >
    <span class="text-muted"><Icon name="sort" size={15} /></span>
    <span class="truncate">{summary}</span>
    <span class="text-muted"><Icon name="caret" size={12} /></span>
  </button>

  {#if open}
    <div
      bind:this={menu}
      class="absolute left-0 top-10 z-30 max-h-[min(70vh,520px)] w-56 overflow-y-auto rounded-box border border-base-300 bg-base-100 p-1.5 text-[13px] shadow-[0_8px_28px_rgb(15_20_30/0.2)]"
      role="menu"
      aria-label="Sort"
      tabindex="-1"
      onclick={(e) => e.stopPropagation()}
      onkeydown={keydown}
    >
      <p class="px-2 pb-1 pt-0.5 text-xs text-muted">Sort by</p>
      {#each keys as k (k)}
        <button role="menuitemradio" aria-checked={app.sort === k} class={item} onclick={() => app.setSort(k)}>
          <span class="w-4 shrink-0">{#if app.sort === k}<Icon name="check" size={14} />{/if}</span>
          <span class="truncate">{label(k)}</span>
        </button>
      {/each}
      <div class="my-1 border-t border-base-300"></div>
      {#each [[false, asc], [true, desc]] as [d, text] (String(d))}
        <button role="menuitemradio" aria-checked={app.sortDesc === d} class={item} onclick={() => (app.sortDesc = d as boolean)}>
          <span class="w-4 shrink-0">{#if app.sortDesc === d}<Icon name="check" size={14} />{/if}</span>
          <span>{text}</span>
        </button>
      {/each}
    </div>
  {/if}
</div>
