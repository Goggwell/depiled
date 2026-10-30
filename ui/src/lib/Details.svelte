<script lang="ts">
  import { coverUrl } from './api';
  import { app, fmtAgo, fmtHours } from './store.svelte';
  import Icon from './Icon.svelte';

  const game = $derived(app.selectedGames.length === 1 ? app.selectedGames[0] : null);
  const ach = $derived(game ? app.achievements[game.appid] : undefined);
  const editable = $derived(app.collections.filter((c) => !c.dynamic));
  const topTags = $derived(game ? game.tags.slice(0, 10) : []);

  let target = $state('');
  async function addMany() {
    if (!target) return;
    await app.addToCollection(app.selected, target);
    target = '';
  }

  const initials = (n: string) =>
    n.split(/\s+/).filter((w) => /\p{L}|\p{N}/u.test(w)).slice(0, 2).map((w) => [...w][0]).join('').toUpperCase() || '?';
  const chip = (on: boolean) => `btn btn-xs h-7 rounded-full px-3 font-medium ${on ? 'btn-primary' : 'btn-soft'}`;
</script>

<aside class="w-80 shrink-0 overflow-y-auto border-l border-base-300 bg-base-100 p-5">
  {#if game}
    <div class="mx-auto mb-4 aspect-[2/3] w-36 overflow-hidden rounded-box shadow-2xl">
      {#if coverUrl(game.cover)}
        <img src={coverUrl(game.cover)} alt="" draggable="false" class="size-full object-cover" />
      {:else}
        <div class="grid size-full place-items-center text-4xl font-bold text-white/85" style="background:linear-gradient(160deg,hsl({(game.appid * 47) % 360} 10% 36%),hsl({((game.appid * 47) % 360) + 40} 10% 22%))">{initials(game.name)}</div>
      {/if}
    </div>
    <h2 class="mb-4 text-center font-display text-xl font-semibold leading-tight">{game.name}</h2>

    <dl class="grid grid-cols-2 gap-2">
      <div class="rounded-field bg-base-200 p-3"><dt class="text-xs text-muted">Playtime</dt><dd class="text-[15px] font-bold tabular-nums">{fmtHours(game.playtime_minutes)}</dd></div>
      <div class="rounded-field bg-base-200 p-3"><dt class="text-xs text-muted">Last played</dt><dd class="text-[15px] font-bold tabular-nums">{fmtAgo(game.last_played)}</dd></div>
      <div class="rounded-field bg-base-200 p-3"><dt class="text-xs text-muted">Reviews</dt><dd class="text-[15px] font-bold tabular-nums">{game.review_percent ? game.review_percent + '% positive' : '–'}</dd></div>
      <div class="rounded-field bg-base-200 p-3"><dt class="text-xs text-muted">Metacritic</dt><dd class="text-[15px] font-bold tabular-nums">{game.metacritic || '–'}</dd></div>
    </dl>

    {#if ach && ach.total > 0}
      <div class="mt-3 rounded-field bg-base-200 p-3">
        <div class="mb-2 flex justify-between text-sm">
          <span class="flex items-center gap-1.5"><Icon name="trophy" size={15} /> Achievements</span>
          <strong>{ach.unlocked} / {ach.total}</strong>
        </div>
        <progress class="progress {ach.unlocked === ach.total ? 'progress-success' : 'progress-primary'} w-full" value={ach.unlocked} max={ach.total}></progress>
      </div>
    {/if}

    <h3 class="mb-2 mt-6 text-xs font-bold uppercase tracking-wider text-muted">In collections</h3>
    <div class="flex flex-wrap gap-1.5">
      {#each editable as c (c.id)}
        <button class={chip(game.collections.includes(c.id))} disabled={app.busy} onclick={() => app.toggleMembership(game.appid, c.id)}>
          {#if game.collections.includes(c.id)}<Icon name="check" size={13} />{/if}
          {c.name}
        </button>
      {/each}
    </div>

    {#if topTags.length}
      <h3 class="mb-2 mt-6 text-xs font-bold uppercase tracking-wider text-muted">Genres and tags</h3>
      <div class="flex flex-wrap gap-1.5">
        {#each topTags as t}
          <button class={chip(app.tagFilter.includes(t))} onclick={() => app.toggleTag(t)} title="Filter by this tag">{app.tagNames[t] ?? t}</button>
        {/each}
      </div>
    {/if}
  {:else if app.selected.length > 1}
    <div class="py-6 text-center">
      <div class="font-display text-6xl font-semibold leading-none">{app.selected.length}</div>
      <p class="mt-1 text-muted">games selected</p>
    </div>
    <h3 class="mb-2 text-xs font-bold uppercase tracking-wider text-muted">Add all to a collection</h3>
    <div class="flex gap-2">
      <select class="select select-sm h-9 min-w-0 flex-1" bind:value={target} aria-label="Collection">
        <option value="" disabled>Choose…</option>
        {#each editable as c (c.id)}<option value={c.id}>{c.name}</option>{/each}
      </select>
      <button class="btn btn-primary btn-sm h-9" disabled={!target || app.busy} onclick={addMany}>Add</button>
    </div>
    {#if app.view !== 'all' && app.view !== 'none'}
      <button class="btn btn-error btn-outline btn-sm mt-3 w-full" disabled={app.busy} onclick={() => app.removeFromCollection(app.selected, app.view)}>
        Remove from {app.viewTitle}
      </button>
    {/if}
    <button class="btn btn-ghost btn-sm mt-2 w-full" onclick={() => app.clearSelection()}>Clear selection</button>
  {:else}
    <div class="flex flex-col items-center gap-2 px-2 py-16 text-center text-muted">
      <Icon name="library" size={30} />
      <p><strong class="text-base-content">Select a game</strong> to see its details, or drag games onto a collection in the sidebar.</p>
      <p class="text-sm">Tip: Ctrl-click or Shift-click to pick several at once.</p>
    </div>
  {/if}
</aside>
