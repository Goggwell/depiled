<script lang="ts">
  // Picks values from a long list (genres and tags, features, collections) by typing part of
  // a name. Arrow keys move through the matches, Enter picks one, and the box stays open so
  // several can be added in a row. The list floats above the page, below the button or above
  // it if there's no room, so the dialog it's in can't cut it off.
  import { tick } from 'svelte';
  import { place, portal } from './float';
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

  const WIDTH = 272;
  const HEIGHT = 320;
  let open = $state(false);
  let query = $state('');
  let active = $state(0);
  let input = $state<HTMLInputElement>();
  let button = $state<HTMLButtonElement>();
  let panel = $state<HTMLDivElement>();
  let pos = $state({ top: 0, left: 0, maxHeight: HEIGHT });

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
    if (open) return close();
    if (button) pos = place(button, WIDTH, HEIGHT);
    open = true;
    query = '';
    active = 0;
    await tick();
    input?.focus();
  }
  function close() {
    open = false;
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
    else if (e.key === 'Escape') {
      close();
      button?.focus();
    } else return;
    e.preventDefault();
    e.stopPropagation();
  }
  $effect(() => {
    void active;
    panel?.querySelector(`[data-i="${active}"]`)?.scrollIntoView({ block: 'nearest' });
  });
  const outside = (t: EventTarget | null) => !panel?.contains(t as Node) && !button?.contains(t as Node);
</script>

<!-- Clicking elsewhere closes it, and so does scrolling what's behind it, which would leave it
     floating away from its button. -->
<svelte:window
  onpointerdown={(e) => open && outside(e.target) && close()}
  onscrollcapture={(e) => open && outside(e.target) && close()}
  onresize={() => open && close()}
/>

<button bind:this={button} class="btn btn-sm h-8 gap-1 border-base-300 bg-base-100 font-normal" onclick={show} aria-haspopup="listbox" aria-expanded={open}>
  <Icon name="plus" size={13} /> {picked.length ? 'Add another' : label}
</button>
{#if open}
  <div
    use:portal
    bind:this={panel}
    role="presentation"
    onclick={(e) => e.stopPropagation()}
    class="fixed z-[1000] flex flex-col rounded-box border border-base-300 bg-base-100 p-2 text-[13px] text-base-content shadow-[0_8px_28px_rgb(15_20_30/0.2)]"
    style="top:{pos.top}px;left:{pos.left}px;width:{WIDTH}px;max-height:{Math.min(HEIGHT, pos.maxHeight)}px"
  >
    <input
      bind:this={input}
      bind:value={query}
      oninput={() => (active = 0)}
      onkeydown={keydown}
      type="search"
      class="input h-10 w-full text-sm"
      placeholder="Type to find"
      aria-label={label}
      aria-controls="picker-list"
      role="combobox"
      aria-expanded="true"
      aria-activedescendant={matches[active] ? `pick-${active}` : undefined}
    />
    <div id="picker-list" class="mt-1.5 min-h-0 flex-1 overflow-y-auto" role="listbox" aria-label={label}>
      {#each matches as m, i (m.v)}
        <button
          id="pick-{i}"
          data-i={i}
          role="option"
          aria-selected={i === active}
          class="flex w-full items-center gap-2 rounded-[4px] px-2 py-1.5 text-left {i === active ? 'bg-base-200' : 'hover:bg-base-200'}"
          onmouseenter={() => (active = i)}
          onclick={() => choose(m.v)}
        >
          <span class="flex-1 truncate">{m.name}</span>
          {#if counts}<span class="text-xs tabular-nums text-muted">{m.n}</span>{/if}
        </button>
      {:else}
        <p class="px-2 py-1.5 text-muted">{query ? 'Nothing matches.' : 'All of them are picked.'}</p>
      {/each}
    </div>
  </div>
{/if}
