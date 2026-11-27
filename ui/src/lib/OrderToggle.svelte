<script lang="ts">
  // Turns "keep my own collection order" on and off, from the Collections heading where the
  // order shows. Opens a short explanation first, since it changes the names in Steam.
  import { tick } from 'svelte';
  import { app } from './store.svelte';
  import Icon from './Icon.svelte';
  import { place, portal } from './float';

  let open = $state(false);
  let button = $state<HTMLButtonElement>();
  let panel = $state<HTMLDivElement>();
  // Floats above the page: the sidebar list scrolls and would cut it off.
  let pos = $state({ top: 0, left: 0 });

  async function toggle() {
    if (!open && button) {
      const p = place(button, 256, 230);
      pos = { top: p.top, left: p.left };
    }
    open = !open;
    if (open) {
      await tick();
      panel?.querySelector<HTMLElement>('.btn-primary, .btn')?.focus();
    }
  }
  async function set(on: boolean) {
    open = false;
    await app.setOrdering(on);
    button?.focus();
  }
</script>

<svelte:window onpointerdown={(e) => open && !panel?.contains(e.target as Node) && !button?.contains(e.target as Node) && (open = false)} />

<span class="relative">
  <button
    bind:this={button}
    class="btn btn-ghost btn-xs btn-square {app.ordered ? 'text-base-content' : ''}"
    aria-label="Collection order"
    aria-expanded={open}
    aria-pressed={app.ordered}
    title={app.ordered ? 'Your own order is on' : 'Keep your own order'}
    onclick={toggle}
  >
    <Icon name="numbers" size={14} weight={app.ordered ? 'bold' : 'regular'} />
  </button>
  {#if open}
    <div
      use:portal
      bind:this={panel}
      style="top:{pos.top}px;left:{pos.left}px"
      class="fixed z-[1000] w-64 rounded-box border border-base-300 bg-base-100 p-3.5 text-[13px] shadow-[0_8px_28px_rgb(15_20_30/0.2)]"
      role="dialog"
      aria-label="Collection order"
      tabindex="-1"
      onkeydown={(e) => {
        if (e.key === 'Escape') {
          open = false;
          button?.focus();
        }
      }}
    >
      {#if app.ordered}
        <p class="font-semibold">Your own order is on</p>
        <p class="mt-1 text-muted">Drag collections in this list to reorder them. In Steam they keep a number in front (01, 02, …) so Steam lists them the same way.</p>
        <button class="btn btn-sm mt-3 w-full" disabled={app.busy || app.saving} onclick={() => set(false)}>Back to alphabetical</button>
      {:else}
        <p class="font-semibold">Keep your own order</p>
        <p class="mt-1 text-muted">Steam always lists collections alphabetically. Depiled can put a number in front of each name in Steam (01 Playing, 02 Backlog) so they stay in the order you choose. Here you’ll see the plain names, and drag them into place.</p>
        <p class="mt-2 text-muted">It reaches Steam when you save, and can be undone.</p>
        <button class="btn btn-primary btn-sm mt-3 w-full" disabled={app.busy || app.saving} onclick={() => set(true)}>Number my collections</button>
      {/if}
    </div>
  {/if}
</span>
