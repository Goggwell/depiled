<script lang="ts">
  import { app } from './store.svelte';
  import { ruleIsEmpty } from './criteria';
  import EmojiPicker from './EmojiPicker.svelte';
  import FilterPanel from './FilterPanel.svelte';

  type Kind = 'empty' | 'selected' | 'auto';

  const editing = $derived(app.dialog === 'rule');
  const target = $derived(app.collections.find((c) => c.id === app.ruleFor));
  const hasRule = $derived(!!(app.ruleFor && app.smartRules[app.ruleFor]));

  let name = $state('');
  let nameInput = $state<HTMLInputElement>();
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
  const hasFamily = $derived((app.library?.games ?? []).some((g) => g.family));
  const newOnes = $derived(target ? app.criteriaMatches.filter((g) => !g.collections.includes(target.id)).length : matches);
  const nameTaken = $derived(app.collections.some((c) => c.name.toLowerCase() === name.trim().toLowerCase()));
  const nothingPicked = $derived(ruleIsEmpty(app.criteria));
  const canSaveRule = $derived(!nothingPicked && !app.unfinishedAdvanced);
  const canCreate = $derived(!app.busy && name.trim().length > 0 && !nameTaken && (kind !== 'auto' || canSaveRule));

  const games = (n: number) => `${n.toLocaleString()} ${n === 1 ? 'game' : 'games'}`;
  const createLabel = $derived(kind === 'auto' ? `Create with ${games(matches)}` : kind === 'selected' ? `Create with ${games(app.selected.length)}` : 'Create');

  function close() {
    app.closeRuleEditor(true);
  }

  async function create() {
    const n = name.trim();
    const id = kind === 'auto' ? await app.createSmart(n, remember) : await app.createCollection(n, kind === 'selected' ? app.selected : []);
    if (!id) return;
    if (kind === 'auto') app.clearFilters();
    app.closeRuleEditor(kind !== 'auto');
    app.view = id;
  }

  async function saveRule() {
    if (!app.ruleFor || !canSaveRule) return;
    if (await app.saveRule(app.ruleFor, app.criteria)) close();
  }

  async function stopAuto() {
    if (!app.ruleFor) return;
    if (await app.saveRule(app.ruleFor, null)) close();
  }

  const options: { value: Kind; title: string; text: string }[] = $derived([
    { value: 'empty', title: 'Start empty', text: 'Add games later by dragging them in.' },
    ...(app.selected.length > 0 ? [{ value: 'selected' as Kind, title: `Use the ${games(app.selected.length)} you picked`, text: 'Put the selected games in it.' }] : []),
    { value: 'auto', title: 'Fill it from filters', text: 'Match games across your whole library, including games outside the current search or collection.' },
  ]);
</script>

{#if app.dialog === 'new' || app.dialog === 'rule'}
  <div class="modal modal-open" role="dialog" aria-label={editing ? 'Auto-fill' : 'New collection'} tabindex="-1" onkeydown={(e) => e.key === 'Escape' && close()}>
    <div class="modal-box flex max-h-[calc(100vh-3rem)] max-w-2xl flex-col rounded-box p-0">
      {#if editing}
        <header class="px-6 pb-3 pt-5">
          <h2 class="font-display text-xl font-semibold">Auto-fill {target?.name}</h2>
          <p class="mt-1 text-[13px] text-muted">Choose what belongs in this collection. Then add everything that fits with one click.</p>
        </header>
        <div class="min-h-0 flex-1 overflow-y-auto px-6 pb-4"><FilterPanel /></div>
        <footer class="flex items-center gap-2 border-t border-base-300 px-6 py-3">
          {#if hasRule}<button class="btn btn-ghost btn-sm text-error" onclick={stopAuto}>Turn off auto-fill</button>{/if}
          <span class="flex-1"></span>
          <span class="text-[13px] tabular-nums text-muted">{matches.toLocaleString()} match, {newOnes.toLocaleString()} not in it yet</span>
          <button class="btn btn-ghost btn-sm" onclick={close}>Cancel</button>
          <button class="btn btn-primary btn-sm" onclick={saveRule} disabled={!canSaveRule || app.busy}>Save</button>
        </footer>
      {:else}
        <header class="px-6 pb-2 pt-5"><h2 class="font-display text-xl font-semibold">New collection</h2></header>
        <div class="min-h-0 flex-1 overflow-y-auto px-6 pb-4">
          <label for="new-name" class="mb-1.5 block text-[13px] font-semibold">Name</label>
          <!-- svelte-ignore a11y_autofocus -->
          <div class="flex items-center gap-1">
            <input id="new-name" type="text" class="input w-full" placeholder="For example Scary nights" bind:value={name} bind:this={nameInput} autofocus onkeydown={(e) => e.key === 'Enter' && canCreate && create()} />
            <EmojiPicker target={nameInput} onpick={(v) => (name = v)} />
          </div>
          {#if nameTaken}<p class="mt-1.5 text-[13px] text-error">You already have a collection with this name.</p>{/if}

          <div class="mt-4 flex flex-col divide-y divide-base-300 rounded-box border border-base-300" role="radiogroup" aria-label="How to fill it">
            {#each options as o (o.value)}
              <label class="flex cursor-pointer items-start gap-3 p-3.5 {kind === o.value ? 'bg-base-200' : 'hover:bg-base-200/60'}">
                <input type="radio" class="radio radio-xs mt-1" bind:group={kind} value={o.value} />
                <span class="flex flex-col text-[13px]"><strong class="font-semibold">{o.title}</strong><span class="text-muted">{o.text}</span></span>
              </label>
            {/each}
          </div>

          {#if kind === 'auto'}
            <div class="mt-4 flex flex-wrap items-center gap-x-4 gap-y-2 text-[13px]">
              <span class="font-semibold tabular-nums" role="status">{nothingPicked ? 'Choose at least one filter below' : `${games(matches)} match right now`}</span>
              <label class="flex cursor-pointer items-center gap-2">
                <input type="checkbox" class="checkbox checkbox-xs" bind:checked={remember} />
                Keep these filters, to add new matches later
              </label>
            </div>
            {#if hasFamily}<p class="mt-1 text-xs text-muted">Family games aren’t included: they come and go as their owner plays them.</p>{/if}
            <div class="mt-3 rounded-box border border-base-300 p-4"><FilterPanel /></div>
          {/if}
        </div>
        <footer class="flex justify-end gap-2 border-t border-base-300 px-6 py-3">
          <button class="btn btn-ghost btn-sm" onclick={close}>Cancel</button>
          <button class="btn btn-primary btn-sm" onclick={create} disabled={!canCreate}>{createLabel}</button>
        </footer>
      {/if}
    </div>
    <button class="modal-backdrop" aria-label="Close" onclick={close}></button>
  </div>
{/if}
