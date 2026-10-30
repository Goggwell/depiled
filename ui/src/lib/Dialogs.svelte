<script lang="ts">
  import * as api from './api';
  import type { OpView, SnapshotInfo } from './api';
  import { app } from './store.svelte';

  let ops = $state<OpView[]>([]);
  let notes = $state<string[]>([]);
  let previewing = $state(false);
  let error = $state<string | null>(null);
  let snaps = $state<SnapshotInfo[]>([]);
  let confirm = $state<string | null>(null);

  const sentence = (o: OpView) => {
    const games = `${o.count} ${o.count === 1 ? 'game' : 'games'}`;
    switch (o.kind) {
      case 'add': return `Add ${games} to ${o.collection}`;
      case 'remove': return `Take ${games} out of ${o.collection}`;
      case 'create': return `Create ${o.collection} with ${games}`;
      case 'rename': return `Rename a collection to ${o.collection}`;
      default: return `Delete ${o.collection}`;
    }
  };

  const kindClass = (k: OpView['kind']) =>
    k === 'remove' || k === 'delete' ? 'badge-error' : k === 'create' ? 'badge-success' : 'badge-info';

  $effect(() => {
    if (app.dialog === 'organize') {
      previewing = true;
      error = null;
      api.previewRules()
        .then((r) => ((ops = r.ops), (notes = r.notes)))
        .catch((e) => (error = String(e)))
        .finally(() => (previewing = false));
    } else if (app.dialog === 'history') {
      confirm = null;
      error = null;
      api.history().then((h) => (snaps = h)).catch((e) => (error = String(e)));
    }
  });

  const when = (ts: number) =>
    new Date(ts * 1000).toLocaleString(undefined, { month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit' });

  async function apply() {
    app.dialog = null;
    await app.applyRules();
  }

  async function restore(s: SnapshotInfo) {
    app.dialog = null;
    await app.restore(s.id, s.label);
  }

  const close = () => (app.dialog = null);
</script>

{#if app.dialog === 'organize'}
  <div class="modal modal-open" role="dialog" aria-label="Auto-organize" tabindex="-1" onkeydown={(e) => e.key === 'Escape' && close()}>
    <div class="modal-box flex max-h-[calc(100vh-3rem)] max-w-xl flex-col p-0">
      <header class="px-6 pb-2 pt-5">
        <h2 class="font-display text-xl font-semibold">Auto-organize your library</h2>
        <p class="mt-1 text-sm text-muted">Review what would change. Nothing happens until you press Apply, and you can undo it afterwards.</p>
      </header>
      <div class="min-h-0 flex-1 overflow-y-auto px-6 py-3">
        {#if previewing}
          <div class="flex items-center gap-3 py-4 text-muted"><span class="loading loading-spinner loading-sm"></span> Checking your library…</div>
        {:else if error}
          <p class="text-error">{error}</p>
        {:else if ops.length === 0}
          <p class="py-2"><strong>Everything is already organized.</strong> No changes needed.</p>
        {:else}
          <ul class="flex flex-col gap-2">
            {#each ops as o}
              <li class="flex items-center gap-3 rounded-field bg-base-200 p-3">
                <span class="badge badge-soft w-16 shrink-0 justify-center font-bold uppercase {kindClass(o.kind)}">{o.kind}</span>
                <div class="min-w-0">
                  <strong>{sentence(o)}</strong>
                  {#if o.sample.length}
                    <div class="mt-0.5 text-[13px] text-muted">{o.sample.join(', ')}{o.count > o.sample.length ? `, and ${o.count - o.sample.length} more` : ''}</div>
                  {/if}
                </div>
              </li>
            {/each}
          </ul>
        {/if}
        {#each notes as n}<div class="alert alert-warning alert-soft mt-3 text-sm">{n}</div>{/each}
      </div>
      <footer class="flex justify-end gap-2 border-t border-base-300 px-6 py-3.5">
        <button class="btn btn-ghost" onclick={close}>Close</button>
        <button class="btn btn-primary" disabled={previewing || !!error || ops.length === 0} onclick={apply}>
          Apply {ops.length} {ops.length === 1 ? 'change' : 'changes'}
        </button>
      </footer>
    </div>
    <button class="modal-backdrop" aria-label="Close" onclick={close}></button>
  </div>
{:else if app.dialog === 'history'}
  <div class="modal modal-open" role="dialog" aria-label="History" tabindex="-1" onkeydown={(e) => e.key === 'Escape' && close()}>
    <div class="modal-box flex max-h-[calc(100vh-3rem)] max-w-xl flex-col p-0">
      <header class="px-6 pb-2 pt-5">
        <h2 class="font-display text-xl font-semibold">History</h2>
        <p class="mt-1 text-sm text-muted">A copy of your collections is saved before every change. Restore one to go back to how things were. Collections made since then are removed, and you can undo that too.</p>
      </header>
      <div class="min-h-0 flex-1 overflow-y-auto px-6 py-3">
        {#if error}
          <p class="text-error">{error}</p>
        {:else if snaps.length === 0}
          <p class="text-muted">No changes yet.</p>
        {:else}
          <ul class="flex flex-col gap-2">
            {#each snaps as s (s.id)}
              <li class="flex items-center justify-between gap-3 rounded-field bg-base-200 p-3">
                <div class="min-w-0">
                  <strong class="block truncate">Before: {s.label}</strong>
                  <div class="text-[13px] text-muted">{when(s.taken_at)}</div>
                </div>
                {#if confirm === s.id}
                  <span class="flex gap-1">
                    <button class="btn btn-primary btn-sm" onclick={() => restore(s)}>Yes, restore</button>
                    <button class="btn btn-ghost btn-sm" onclick={() => (confirm = null)}>Cancel</button>
                  </span>
                {:else}
                  <button class="btn btn-sm" onclick={() => (confirm = s.id)}>Restore</button>
                {/if}
              </li>
            {/each}
          </ul>
        {/if}
      </div>
      <footer class="flex justify-end border-t border-base-300 px-6 py-3.5"><button class="btn" onclick={close}>Close</button></footer>
    </div>
    <button class="modal-backdrop" aria-label="Close" onclick={close}></button>
  </div>
{/if}
