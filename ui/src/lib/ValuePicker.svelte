<script lang="ts">
  // Picks values from a long list (genres and tags, features, collections) by typing part of
  // a name. Arrow keys move through the matches, Enter picks one, and the box stays open so
  // several can be added in a row.
  import { tick } from 'svelte';
  import { fold } from './store.svelte';
  import Icon from './Icon.svelte';

  type Value = string | number;
  let {
    options,
    picked,
    onpick,
    counts,
    label = 'Add a value',
  }: { options: [Value, string][]; picked: Value[]; onpick: (v: Value) => void; counts?: Map<Value, number>; label?: string } = $props();

  let open = $state(false);
  let query = $state('');
  let active = $state(0);
  let input = $state<HTMLInputElement>();
  let root = $state<HTMLDivElement>();

  const matches = $derived.by(() => {
    const q = fold(query.trim());
    const left = options.filter(([v, name]) => !picked.includes(v) && (!q || fold(name).includes(q)));
    // Best matches first: names that start with what was typed, then the most common.
    return left
      .map(([v, name]) => ({ v, name, starts: q ? fold(name).startsWith(q) : false, n: counts?.get(v) ?? 0 }))
      .sort((a, b) => Number(b.starts) - Number(a.starts) || b.n - a.n || a.name.localeCompare(b.name))
      .slice(0, 80);
  });

  async function show() {
    open = true;
    query = '';
    active = 0;
    await tick();
    input?.focus();
  }
  function choose(v: Value) {
    onpick(v);
    query = '';
    active = 0;
    input?.focus();
  }
  function keydown(e: KeyboardEvent) {
    if (e.key === 'ArrowDown') active = Math.min(active + 1, matches.length - 1);
    else if (e.key === 'ArrowUp') active = Math.max(active - 1, 0);
    else if (e.key === 'Enter' && matches[active]) choose(matches[active].v);
    else if (e.key === 'Escape') open = false;
    else return;
    e.preventDefault();
    e.stopPropagation();
  }
  $effect(() => {
    void active;
    root?.querySelector(`[data-i="${active}"]`)?.scrollIntoView({ block: 'nearest' });
  });
</script>

<svelte:window onclick={(e) => open && !root?.contains(e.target as Node) && (open = false)} />

<div class="relative" bind:this={root}>
  <button class="btn btn-xs h-7 gap-1 border-base-300 bg-base-100 font-normal" onclick={show} aria-haspopup="listbox" aria-expanded={open}>
    <Icon name="plus" size={12} /> {picked.length ? 'Add another' : label}
  </button>
  {#if open}
    <div class="absolute left-0 top-8 z-30 w-64 rounded-box border border-base-300 bg-base-100 p-1.5 shadow-[0_8px_28px_rgb(15_20_30/0.2)]">
      <input
        bind:this={input}
        bind:value={query}
        oninput={() => (active = 0)}
        onkeydown={keydown}
        type="search"
        class="input input-xs h-7 w-full"
        placeholder="Type to find"
        aria-label={label}
        aria-controls="picker-list"
        role="combobox"
        aria-expanded="true"
        aria-activedescendant={matches[active] ? `pick-${active}` : undefined}
      />
      <div id="picker-list" class="mt-1 max-h-60 overflow-y-auto" role="listbox" aria-label={label}>
        {#each matches as m, i (m.v)}
          <button
            id="pick-{i}"
            data-i={i}
            role="option"
            aria-selected={i === active}
            class="flex w-full items-center gap-2 rounded-[4px] px-2 py-1 text-left text-xs {i === active ? 'bg-base-200' : 'hover:bg-base-200'}"
            onmouseenter={() => (active = i)}
            onclick={() => choose(m.v)}
          >
            <span class="flex-1 truncate">{m.name}</span>
            {#if counts}<span class="tabular-nums text-muted">{m.n}</span>{/if}
          </button>
        {:else}
          <p class="px-2 py-1.5 text-xs text-muted">{query ? 'Nothing matches.' : 'All of them are picked.'}</p>
        {/each}
      </div>
    </div>
  {/if}
</div>
