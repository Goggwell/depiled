<script lang="ts">
  // An emoji button for a name field: opens a searchable grid, and puts the chosen emoji where
  // the cursor was in the field (or at the start if the field wasn't touched). The grid floats
  // above the page, so the dialog or list it opens from can't cut it off or stretch.
  import { tick } from 'svelte';
  import { EMOJI } from './emojis';
  import { place, portal } from './float';
  import { fold } from './store.svelte';

  let {
    target,
    onpick,
    ondismiss,
  }: {
    target: HTMLInputElement | undefined;
    onpick: (value: string) => void;
    /** Closed by clicking somewhere else, the way clicking out of the field would end editing. */
    ondismiss?: () => void;
  } = $props();

  const WIDTH = 296;
  const HEIGHT = 340;
  let open = $state(false);
  let query = $state('');
  let search = $state<HTMLInputElement>();
  let button = $state<HTMLButtonElement>();
  let panel = $state<HTMLDivElement>();
  let pos = $state({ top: 0, left: 0, maxHeight: HEIGHT });
  // Where the cursor was in the name field when the picker opened.
  let caret: [number, number] | null = null;

  const groups = $derived.by(() => {
    const q = fold(query.trim());
    if (!q) return EMOJI;
    const seen = new Set<string>();
    const items = EMOJI.flatMap((g) => g.items).filter(([e, words]) => !seen.has(e) && words.split(' ').some((w) => w.startsWith(q)) && seen.add(e));
    return [{ name: 'Matches', items }];
  });

  async function toggle() {
    if (open) {
      open = false;
      target?.focus();
      return;
    }
    if (target) caret = target.selectionStart != null && document.activeElement === target ? [target.selectionStart, target.selectionEnd ?? target.selectionStart] : null;
    if (button) pos = place(button, WIDTH, HEIGHT, 'right');
    open = true;
    query = '';
    await tick();
    search?.focus();
  }

  async function pick(e: string) {
    if (!target) return;
    const value = target.value;
    // Into the name where the cursor was; otherwise in front, followed by a space.
    const [from, to] = caret ?? [0, 0];
    const insert = caret ? e : `${e} `;
    const next = value.slice(0, from) + insert + value.slice(to);
    onpick(next);
    open = false;
    await tick();
    target.focus();
    const at = from + insert.length;
    target.setSelectionRange(at, at);
  }

  function outsideClick(e: PointerEvent) {
    if (!open || panel?.contains(e.target as Node) || button?.contains(e.target as Node)) return;
    open = false;
    // Back in the name field: carry on editing. Anywhere else: done, like leaving the field.
    if (!target?.contains(e.target as Node)) ondismiss?.();
  }
</script>

<svelte:window onpointerdown={outsideClick} onresize={() => (open = false)} />

<button bind:this={button} type="button" class="btn btn-ghost btn-sm btn-square text-base" aria-label="Add an emoji" title="Add an emoji" aria-expanded={open} onmousedown={(e) => e.preventDefault()} onclick={toggle}>🙂</button>
{#if open}
  <div
    use:portal
    bind:this={panel}
    style="top:{pos.top}px;left:{pos.left}px;width:{WIDTH}px;max-height:{Math.min(HEIGHT, pos.maxHeight)}px"
    class="fixed z-[1000] flex flex-col rounded-box border border-base-300 bg-base-100 p-2 text-base-content shadow-[0_8px_28px_rgb(15_20_30/0.2)]"
    role="dialog"
    aria-label="Emoji"
    tabindex="-1"
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => {
      if (e.key === 'Escape') {
        e.stopPropagation();
        open = false;
        target?.focus();
      }
    }}
  >
    <input bind:this={search} bind:value={query} type="search" class="input h-10 w-full text-sm" placeholder="Find an emoji, like trophy or cozy" aria-label="Find an emoji" />
    <div class="mt-1.5 min-h-0 flex-1 overflow-y-auto">
      {#each groups as g (g.name)}
        <p class="px-0.5 pb-0.5 pt-1.5 text-xs text-muted">{g.name}</p>
        <div class="grid grid-cols-8">
          {#each g.items as [e, words] (e + g.name)}
            <button type="button" class="grid size-8 place-items-center rounded-[4px] text-lg hover:bg-base-200 focus-visible:bg-base-200 focus-visible:outline-none" title={words.split(' ')[0]} aria-label={words.split(' ')[0]} onclick={() => pick(e)}>{e}</button>
          {/each}
        </div>
      {/each}
      {#if groups.length && !groups[0].items.length}<p class="px-1 py-2 text-xs text-muted">No emoji for “{query}”.</p>{/if}
    </div>
  </div>
{/if}
