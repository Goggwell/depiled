<script lang="ts">
  import type { Snippet } from 'svelte';
  import * as api from './api';
  import type { OpView, SnapshotInfo } from './api';
  import { app, NO_COLOR } from './store.svelte';
  import Icon from './Icon.svelte';

  let ops = $state<OpView[]>([]);
  let notes = $state<string[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let snaps = $state<SnapshotInfo[]>([]);
  let confirm = $state<string | null>(null);
  let mergeFrom = $state<string[]>([]);

  const sentence = (o: OpView) => {
    const games = `${o.count.toLocaleString()} ${o.count === 1 ? 'game' : 'games'}`;
    switch (o.kind) {
      case 'add': return `Add ${games} to ${o.collection}`;
      case 'remove': return `Take ${games} out of ${o.collection}`;
      case 'create': return `Create ${o.collection} with ${games}`;
      case 'rename': return `Rename a collection to ${o.collection}`;
      default: return `Delete ${o.collection}`;
    }
  };
  /** Games the rules would move, counting each game once per collection it goes in or out of. */
  const moved = $derived(ops.reduce((n, o) => n + (o.kind === 'add' || o.kind === 'remove' || o.kind === 'create' ? o.count : 0), 0));
  const mark = (k: OpView['kind']) => (k === 'remove' || k === 'delete' ? 'bg-error' : k === 'create' || k === 'add' ? 'bg-success' : 'bg-info');

  $effect(() => {
    error = null;
    confirm = null;
    if (app.dialog === 'organize') {
      loading = true;
      api.previewRules().then((r) => ((ops = r.ops), (notes = r.notes))).catch((e) => (error = String(e))).finally(() => (loading = false));
    } else if (app.dialog === 'save') {
      loading = true;
      notes = [];
      api.pending().then((p) => (ops = p)).catch((e) => (error = String(e))).finally(() => (loading = false));
    } else if (app.dialog === 'history') {
      api.history().then((h) => (snaps = h)).catch((e) => (error = String(e)));
    } else if (app.dialog === 'merge') {
      mergeFrom = [];
    }
  });

  const when = (ts: number) => new Date(ts * 1000).toLocaleString(undefined, { month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit' });
  const close = () => (app.dialog = null);

  async function organize() {
    close();
    await app.applyRules();
  }

  async function save() {
    const running = app.draft.steam_running;
    close();
    await app.save(running);
  }

  async function restore(s: SnapshotInfo) {
    close();
    await app.restore(s.id);
  }

  const into = $derived(app.collections.find((c) => c.id === app.mergeInto));
  const others = $derived(app.collections.filter((c) => !c.dynamic && c.id !== app.mergeInto));
  const mergedCount = $derived.by(() => {
    if (!into) return 0;
    const ids = new Set([into.id, ...mergeFrom]);
    return (app.library?.games ?? []).filter((g) => g.collections.some((c) => ids.has(c))).length;
  });

  async function merge() {
    if (!into || !mergeFrom.length) return;
    close();
    await app.mergeCollections(mergeFrom, into.id);
  }
</script>

{#snippet shell(label: string, title: string, intro: string, body: Snippet, actions: Snippet)}
  <div class="modal modal-open" role="dialog" aria-label={label} tabindex="-1" onkeydown={(e) => e.key === 'Escape' && close()}>
    <div class="modal-box flex max-h-[calc(100vh-3rem)] max-w-xl flex-col rounded-box p-0">
      <header class="px-6 pb-3 pt-5">
        <h2 class="font-display text-xl font-semibold">{title}</h2>
        <p class="mt-1 text-[13px] text-muted">{intro}</p>
      </header>
      <div class="min-h-0 flex-1 overflow-y-auto px-6 pb-4">{@render body()}</div>
      <footer class="flex items-center justify-end gap-2 border-t border-base-300 px-6 py-3">{@render actions()}</footer>
    </div>
    <button class="modal-backdrop" aria-label="Close" onclick={close}></button>
  </div>
{/snippet}

{#snippet opList()}
  {#if loading}
    <div class="flex items-center gap-3 py-4 text-muted"><span class="loading loading-spinner loading-sm"></span> Checking your library…</div>
  {:else if error}
    <p class="rounded-[4px] bg-error/10 p-3 text-[13px]"><strong class="font-semibold">Couldn’t read your collections.</strong> {error} Close this and try again; if it keeps happening, restart Depiled.</p>
  {:else if ops.length === 0}
    <p class="py-2">Nothing to change. Your library already matches.</p>
    {#if !Object.keys(app.smartRules).length}
      <p class="text-[13px] text-muted">None of your collections fill themselves yet. Give one criteria with Auto-fill in its menu, or start from a preset.</p>
    {/if}
  {:else}
    <ul class="divide-y divide-base-300 rounded-box border border-base-300">
      {#each ops as o}
        <li class="flex gap-3 px-3.5 py-2.5">
          <span class="mt-1.5 size-2 shrink-0 rounded-full {mark(o.kind)}"></span>
          <div class="min-w-0 text-[13px]">
            <strong class="font-semibold">{sentence(o)}</strong>
            {#if o.sample.length}<div class="mt-0.5 text-muted">{o.sample.join(', ')}{o.count > o.sample.length ? `, and ${(o.count - o.sample.length).toLocaleString()} more` : ''}</div>{/if}
          </div>
        </li>
      {/each}
    </ul>
  {/if}
  {#each notes as n}<p class="mt-3 flex gap-2 rounded-field bg-base-200 p-3 text-[13px]"><Icon name="warning" size={16} /> {n}</p>{/each}
{/snippet}

{#if app.dialog === 'organize'}
  {#snippet body()}{@render opList()}{/snippet}
  {#snippet actions()}
    <button class="btn btn-ghost btn-sm mr-auto" onclick={() => (app.dialog = 'presets')}><Icon name="wand" size={15} /> Presets…</button>
    <button class="btn btn-ghost btn-sm" onclick={close}>Close</button>
    <button class="btn btn-primary btn-sm" disabled={loading || !!error || ops.length === 0} onclick={organize}>{moved ? `Move ${moved.toLocaleString()} ${moved === 1 ? 'game' : 'games'}` : 'Apply'}</button>
  {/snippet}
  {@render shell('Auto-organize', 'Auto-organize', 'Tops up your collections that fill themselves, and runs your rules for the collections you have. It never makes new collections; for a ready-made set, pick a preset. This is what would change. Nothing goes to Steam until you save.', body, actions)}
{:else if app.dialog === 'save'}
  {#snippet body()}
    {@render opList()}
    {#if app.draft.steam_running}
      <div class="mt-4 rounded-box border border-base-300 p-3.5 text-[13px]">
        <p class="font-semibold">Steam has to close while saving</p>
        <p class="mt-0.5 text-muted">Steam rewrites its collection file while it runs. Depiled closes Steam, saves, and {app.settings.reopen_steam ? 'opens it again' : 'leaves it closed'}. Steam then syncs the changes to your other devices.</p>
        <label class="mt-2.5 flex cursor-pointer items-center gap-2">
          <input type="checkbox" class="checkbox checkbox-xs" checked={app.settings.reopen_steam} onchange={(e) => app.updateSettings({ reopen_steam: e.currentTarget.checked })} />
          Open Steam again afterwards
        </label>
      </div>
    {/if}
    <p class="mt-3 text-xs text-muted">A copy of your current collections is kept first, so you can go back from History.</p>
  {/snippet}
  {#snippet actions()}
    <button class="btn btn-ghost btn-sm" onclick={close}>Keep editing</button>
    <button class="btn btn-primary btn-sm" disabled={loading || !!error || ops.length === 0} onclick={save}>
      <Icon name="save" size={15} /> {app.draft.steam_running ? 'Close Steam and save' : 'Save to Steam'}
    </button>
  {/snippet}
  {@render shell('Save to Steam', 'Save to Steam', 'These changes will be written to Steam’s collection file.', body, actions)}
{:else if app.dialog === 'history'}
  {#snippet body()}
    {#if error}
      <p class="rounded-[4px] bg-error/10 p-3 text-[13px]"><strong class="font-semibold">Couldn’t read the saved copies.</strong> {error}</p>
    {:else if snaps.length === 0}
      <p class="text-muted">Nothing saved yet. A copy appears here each time you save to Steam.</p>
    {:else}
      <ul class="divide-y divide-base-300 rounded-box border border-base-300">
        {#each snaps as s, i (s.id)}
          <li class="flex items-center justify-between gap-3 px-3.5 py-2.5 text-[13px]">
            <div class="min-w-0">
              <strong class="block truncate font-semibold">{when(s.taken_at)}{i === 0 ? ', the latest' : ''}</strong>
              <span class="text-muted">Before {s.label.replace(/^before[- ]/, '')}: {s.collections} {s.collections === 1 ? 'collection' : 'collections'}{s.games != null ? ` holding ${s.games.toLocaleString()} ${s.games === 1 ? 'game' : 'games'}` : ''}</span>
            </div>
            {#if confirm === s.id}
              <span class="flex gap-1">
                <button class="btn btn-primary btn-xs" onclick={() => restore(s)}>Restore</button>
                <button class="btn btn-ghost btn-xs" onclick={() => (confirm = null)}>Cancel</button>
              </span>
            {:else}
              <button class="btn btn-xs" onclick={() => (confirm = s.id)}>Restore…</button>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  {/snippet}
  {#snippet actions()}<button class="btn btn-sm" onclick={close}>Close</button>{/snippet}
  {@render shell('History', 'Earlier versions', 'A copy of your collections from before each save. Restoring one puts it in your unsaved edits; save to send it to Steam.', body, actions)}
{:else if app.dialog === 'merge' && into}
  {#snippet body()}
    <div class="flex flex-col">
      {#each others as c (c.id)}
        <label class="flex h-9 cursor-pointer items-center gap-2.5 rounded-field px-2 text-[13px] hover:bg-base-200">
          <input type="checkbox" class="checkbox checkbox-xs" value={c.id} bind:group={mergeFrom} />
          <span class="spine" style="background:{c.color ?? NO_COLOR};opacity:{c.color ? 1 : 0.35}"></span>
          <span class="flex-1 truncate">{c.name}</span>
          <span class="tabular-nums text-muted">{c.count.toLocaleString()}</span>
        </label>
      {/each}
    </div>
  {/snippet}
  {#snippet actions()}
    {#if mergeFrom.length}<span class="mr-auto text-[13px] text-muted">{into.name} will hold {mergedCount.toLocaleString()} games</span>{/if}
    <button class="btn btn-ghost btn-sm" onclick={close}>Cancel</button>
    <button class="btn btn-primary btn-sm" disabled={!mergeFrom.length} onclick={merge}>Merge {mergeFrom.length || ''} into {into.name}</button>
  {/snippet}
  {@render shell('Merge', `Merge into ${into.name}`, 'Their games move into this collection, and the merged collections are deleted. Undo brings them back.', body, actions)}
{/if}
