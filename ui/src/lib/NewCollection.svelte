<script lang="ts">
  import { app } from './store.svelte';
  import FilterPanel from './FilterPanel.svelte';

  type Kind = 'empty' | 'selected' | 'auto';

  const editing = $derived(app.dialog === 'rule');
  const target = $derived(app.collections.find((c) => c.id === app.ruleFor));
  const hasRule = $derived(!!(app.ruleFor && app.smartRules[app.ruleFor]));

  let name = $state('');
  let kind = $state<Kind>('empty');
  let remember = $state(true);
  let wasOpen = false;

  // Pick sensible starting choices each time the dialog opens.
  $effect(() => {
    const open = app.dialog === 'new' || app.dialog === 'rule';
    if (open && !wasOpen) {
      name = '';
      remember = true;
      kind = app.activeFilterCount > 0 ? 'auto' : app.selected.length > 0 ? 'selected' : 'empty';
    }
    wasOpen = open;
  });

  const matches = $derived(app.criteriaMatches.length);
  const newOnes = $derived(target ? app.criteriaMatches.filter((g) => !g.collections.includes(target.id)).length : matches);
  const nameTaken = $derived(app.collections.some((c) => c.name.toLowerCase() === name.trim().toLowerCase()));
  const nothingPicked = $derived(app.activeFilterCount === 0);

  const canCreate = $derived(!app.busy && name.trim().length > 0 && !nameTaken && (kind !== 'auto' || !nothingPicked));

  const createLabel = $derived(
    kind === 'auto'
      ? `Create with ${matches} ${matches === 1 ? 'game' : 'games'}`
      : kind === 'selected'
        ? `Create with ${app.selected.length} ${app.selected.length === 1 ? 'game' : 'games'}`
        : 'Create',
  );

  function close() {
    if (editing) app.clearFilters();
    app.dialog = null;
    app.ruleFor = null;
  }

  async function create() {
    const n = name.trim();
    const ok = kind === 'auto' ? await app.createSmart(n, remember) : await app.createCollection(n, kind === 'selected' ? app.selected : []);
    if (!ok) return;
    const made = app.collections.find((c) => c.name.toLowerCase() === n.toLowerCase());
    if (kind === 'auto') app.clearFilters();
    app.dialog = null;
    if (made) app.view = made.id;
  }

  async function saveRule() {
    if (!app.ruleFor || nothingPicked) return;
    if (await app.saveRule(app.ruleFor, app.criteria)) close();
  }

  async function stopAuto() {
    if (!app.ruleFor) return;
    if (await app.saveRule(app.ruleFor, null)) close();
  }

  const options: { value: Kind; title: string; text: string }[] = $derived([
    { value: 'empty', title: 'Start empty', text: 'Add games later by dragging them in.' },
    ...(app.selected.length > 0
      ? [{ value: 'selected' as Kind, title: `Use the ${app.selected.length} selected ${app.selected.length === 1 ? 'game' : 'games'}`, text: 'Put the games you picked in it.' }]
      : []),
    { value: 'auto', title: 'Fill it automatically', text: 'Choose filters like genre or play status. Matching games are added now, and you can add new matches any time.' },
  ]);
</script>

{#if app.dialog === 'new' || app.dialog === 'rule'}
  <div class="modal modal-open" role="dialog" aria-label={editing ? 'Auto-fill' : 'New collection'} tabindex="-1" onkeydown={(e) => e.key === 'Escape' && close()}>
    <div class="modal-box flex max-h-[calc(100vh-3rem)] max-w-2xl flex-col p-0">
      {#if editing}
        <header class="px-6 pb-2 pt-5">
          <h2 class="font-display text-xl font-semibold">Auto-fill “{target?.name}”</h2>
          <p class="mt-1 text-sm text-muted">Choose what belongs in this collection. You can then add everything that fits with one click.</p>
        </header>
        <div class="min-h-0 flex-1 overflow-y-auto px-6 py-4"><FilterPanel /></div>
        <footer class="flex items-center gap-2 border-t border-base-300 px-6 py-3.5">
          {#if hasRule}<button class="btn btn-error btn-outline btn-sm" onclick={stopAuto} disabled={app.busy}>Turn off auto-fill</button>{/if}
          <span class="flex-1"></span>
          <span class="text-sm text-muted">{matches} match, {newOnes} not in it yet</span>
          <button class="btn btn-ghost" onclick={close}>Cancel</button>
          <button class="btn btn-primary" onclick={saveRule} disabled={app.busy || nothingPicked}>Save</button>
        </footer>
      {:else}
        <header class="px-6 pb-1 pt-5"><h2 class="font-display text-xl font-semibold">New collection</h2></header>
        <div class="min-h-0 flex-1 overflow-y-auto px-6 py-3">
          <!-- svelte-ignore a11y_autofocus -->
          <input type="text" class="input input-lg w-full text-base" placeholder="Name, for example “Scary nights”" bind:value={name} autofocus onkeydown={(e) => e.key === 'Enter' && canCreate && create()} />
          {#if nameTaken}<p class="mt-1.5 text-sm text-error">You already have a collection with this name.</p>{/if}

          <div class="mt-4 flex flex-col gap-2" role="radiogroup" aria-label="How to fill it">
            {#each options as o (o.value)}
              <label class="flex cursor-pointer items-start gap-3 rounded-box border p-3.5 transition-colors {kind === o.value ? 'border-secondary bg-base-200' : 'border-base-300 hover:bg-base-200/60'}">
                <input type="radio" class="radio radio-primary radio-sm mt-1" bind:group={kind} value={o.value} />
                <span class="flex flex-col"><strong>{o.title}</strong><span class="text-sm text-muted">{o.text}</span></span>
              </label>
            {/each}
          </div>

          {#if kind === 'auto'}
            <div class="mt-4 rounded-box border border-base-300 p-4"><FilterPanel /></div>
            <label class="mt-3 flex cursor-pointer items-center gap-3 text-sm">
              <input type="checkbox" class="checkbox checkbox-primary checkbox-sm" bind:checked={remember} />
              Remember these filters so I can add new matching games later
            </label>
            {#if nothingPicked}<p class="mt-2 text-sm text-muted">Pick at least one filter above.</p>{/if}
          {/if}
        </div>
        <footer class="flex justify-end gap-2 border-t border-base-300 px-6 py-3.5">
          <button class="btn btn-ghost" onclick={close}>Cancel</button>
          <button class="btn btn-primary" onclick={create} disabled={!canCreate}>{createLabel}</button>
        </footer>
      {/if}
    </div>
    <button class="modal-backdrop" aria-label="Close" onclick={close}></button>
  </div>
{/if}
