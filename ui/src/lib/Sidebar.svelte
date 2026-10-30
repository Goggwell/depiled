<script lang="ts">
  import { app } from './store.svelte';
  import Icon from './Icon.svelte';

  let dropTarget = $state<string | null>(null);
  let renaming = $state<string | null>(null);
  let renameValue = $state('');
  let confirmDelete = $state<string | null>(null);

  const total = $derived(app.library?.games.length ?? 0);

  function accepts(e: DragEvent) {
    return e.dataTransfer?.types.includes('application/x-games') ?? false;
  }

  function over(e: DragEvent, id: string) {
    if (!accepts(e)) return;
    e.preventDefault();
    e.dataTransfer!.dropEffect = e.shiftKey ? 'move' : 'copy';
    dropTarget = id;
  }

  function drop(e: DragEvent, id: string) {
    dropTarget = null;
    const raw = e.dataTransfer?.getData('application/x-games');
    if (!raw) return;
    e.preventDefault();
    app.addToCollection(JSON.parse(raw) as number[], id, e.shiftKey);
  }

  async function commitRename() {
    const id = renaming;
    const name = renameValue.trim();
    renaming = null;
    if (id && name) await app.renameCollection(id, name);
  }

  function startRename(id: string, name: string) {
    renaming = id;
    renameValue = name;
  }

  async function remove(id: string) {
    confirmDelete = null;
    await app.deleteCollection(id);
  }

  const focus = (node: HTMLElement) => node.focus();

  const base = 'group relative flex h-9 w-full items-center gap-2 rounded-field px-2.5 text-left text-sm transition-colors';
</script>

<aside class="flex w-64 shrink-0 flex-col gap-3 border-r border-base-300 bg-base-100 p-3">
  <div class="flex items-center gap-3 px-1.5 pb-1 pt-2">
    <div class="grid size-10 place-items-center rounded-box bg-primary text-primary-content"><Icon name="library" size={22} /></div>
    <div class="min-w-0">
      <div class="font-display text-lg font-semibold leading-tight">Collections</div>
      <div class="text-xs text-muted">{total} games</div>
    </div>
  </div>

  <nav class="flex min-h-0 flex-1 flex-col gap-0.5 overflow-y-auto pr-0.5">
    <button class="{base} {app.view === 'all' ? 'bg-base-300 font-semibold' : 'hover:bg-base-200'}" onclick={() => (app.view = 'all')}>
      <Icon name="library" size={17} /> <span class="flex-1 truncate">All games</span>
      <span class="text-xs text-muted">{total}</span>
    </button>
    <button class="{base} {app.view === 'none' ? 'bg-base-300 font-semibold' : 'hover:bg-base-200'}" onclick={() => (app.view = 'none')}>
      <Icon name="inbox" size={17} /> <span class="flex-1 truncate">Not in a collection</span>
      <span class="text-xs text-muted">{app.uncategorizedCount}</span>
    </button>

    <div class="mb-1 mt-4 px-2.5 text-xs font-bold uppercase tracking-wider text-muted">Your collections</div>

    {#each app.collections as c (c.id)}
      <div
        class="{base} {app.view === c.id ? 'bg-base-300 font-semibold' : 'hover:bg-base-200'} {dropTarget === c.id ? 'outline-2 -outline-offset-2 outline-dashed outline-secondary bg-base-300' : ''} {c.dynamic ? 'opacity-70' : ''}"
        role="listitem"
        ondragover={(e) => !c.dynamic && over(e, c.id)}
        ondragleave={() => (dropTarget = null)}
        ondrop={(e) => !c.dynamic && drop(e, c.id)}
      >
        {#if renaming === c.id}
          <Icon name="folder" size={17} />
          <input
            type="text"
            class="input input-sm h-7 min-w-0 flex-1"
            bind:value={renameValue}
            use:focus
            onblur={commitRename}
            onkeydown={(e) => {
              if (e.key === 'Enter') commitRename();
              if (e.key === 'Escape') renaming = null;
            }}
          />
        {:else}
          <button
            class="flex h-full min-w-0 flex-1 items-center gap-2 text-left"
            disabled={c.dynamic}
            title={c.dynamic ? "Managed by Steam, so it can't be changed here" : ''}
            onclick={() => (app.view = c.id)}
            ondblclick={() => !c.dynamic && startRename(c.id, c.name)}
          >
            <Icon name={c.dynamic ? 'lock' : 'folder'} size={17} />
            <span class="truncate">{c.name}</span>
          </button>

          {#if confirmDelete === c.id}
            <button class="btn btn-error btn-xs" onclick={() => remove(c.id)}>Delete?</button>
            <button class="btn btn-ghost btn-xs" onclick={() => (confirmDelete = null)}>No</button>
          {:else}
            <span class="text-xs text-muted group-hover:hidden group-focus-within:hidden">{c.count}</span>
            {#if !c.dynamic}
              <span class="hidden items-center group-hover:flex group-focus-within:flex">
                <button class="btn btn-ghost btn-xs btn-square" aria-label="Rename" title="Rename" onclick={() => startRename(c.id, c.name)}><Icon name="edit" size={14} /></button>
                <button class="btn btn-ghost btn-xs btn-square" aria-label="Delete" title="Delete" onclick={() => (confirmDelete = c.id)}><Icon name="trash" size={14} /></button>
              </span>
            {/if}
          {/if}
        {/if}
      </div>
    {/each}
  </nav>

  <button class="btn btn-sm btn-soft" onclick={() => (app.dialog = 'new')}><Icon name="plus" size={16} /> New collection</button>
  <p class="px-1 text-xs leading-relaxed text-muted">Drag games onto a collection to add them. Hold Shift to move instead.</p>
</aside>
