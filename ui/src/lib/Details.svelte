<script lang="ts">
  import { coverUrl } from './api';
  import { app, fmtAgo, fmtHours, fmtSize, fmtSpan, fmtYear, NO_COLOR } from './store.svelte';
  import Icon from './Icon.svelte';
  import { ACHIEVEMENTS_FEATURE } from './criteria';

  const game = $derived(app.selectedGames.length === 1 ? app.selectedGames[0] : null);
  const ach = $derived(game?.achievements);
  const hunt = $derived(game?.hunters && game.hunters.median_minutes > 0 ? game.hunters : null);
  const editable = $derived(app.collections.filter((c) => !c.dynamic));
  const topTags = $derived(game ? game.tags.slice(0, 10) : []);
  // Steam's store features, in the order Steam lists them, apart from player tags.
  const features = $derived(game ? (app.library?.flags ?? []).filter((f) => game.categories.includes(f.id)) : []);

  let target = $state('');
  async function addMany() {
    if (!target) return;
    await app.addToCollection(app.selected, target);
    target = '';
  }

  const pct = (n: number, d: number) => (d ? Math.round((n / d) * 100) : 0);
  const DECK = ['Unknown', 'Unsupported', 'Playable', 'Verified'];
  const platforms = (bits: number) => [['Windows', 1], ['macOS', 2], ['Linux', 4]].filter(([, b]) => bits & (b as number)).map(([n]) => n).join(', ') || 'Unknown';
</script>

{#snippet fact(label: string, value: string)}
  <div class="flex items-baseline justify-between gap-3 py-1.5">
    <dt class="text-muted">{label}</dt>
    <dd class="text-right font-medium tabular-nums">{value}</dd>
  </div>
{/snippet}

<!-- Only there while something is picked, so covers and the table get the whole width the rest
     of the time. Docked on wide windows; over the list on narrow ones. -->
{#if app.selected.length}
<aside aria-label="Game details" class="w-80 shrink-0 overflow-y-auto border-l border-base-300 bg-base-100 max-xl:absolute max-xl:inset-y-0 max-xl:right-0 max-xl:z-20 max-xl:shadow-[0_8px_32px_rgb(15_20_30/0.22)]">
  <div class="sticky top-0 z-10 flex justify-end bg-base-100 px-2 pt-2">
    <button class="btn btn-ghost btn-sm btn-square" aria-label="Close details (Esc)" title="Close (Esc)" onclick={() => app.clearSelection()}><Icon name="close" size={16} /></button>
  </div>
  {#if game}
    <div class="flex gap-4 p-5 pb-4">
      <div class="aspect-[2/3] w-24 shrink-0 overflow-hidden rounded-[6px] bg-base-300 shadow-md">
        {#if coverUrl(game.cover)}<img src={coverUrl(game.cover)} alt="" draggable="false" class="size-full object-cover" />{/if}
      </div>
      <div class="min-w-0 pt-1">
        <h2 class="font-display text-lg font-semibold leading-tight">{game.name}</h2>
        <p class="mt-1 text-[13px] text-muted">{[game.developers[0], fmtYear(game.release)].filter((x) => x && x !== 'Unknown').join(', ')}</p>
        {#if game.family}
          <p class="mt-2 inline-flex items-center gap-1.5 rounded-[4px] bg-info px-2 py-0.5 text-xs font-semibold text-info-content"><Icon name="family" size={13} weight="bold" /> Family game</p>
          <p class="mt-1 text-xs text-muted">Lent by someone in your Steam Family, so you can play it while they aren’t.</p>
        {/if}
      </div>
    </div>

    <!-- Filing the game is what the app is for, so its collections come first. -->
    <section class="border-t border-base-300 px-5 py-3">
      <h3 class="mb-1.5 text-[13px] font-semibold">Collections</h3>
      {#if editable.length}
        <div class="flex flex-col">
          {#each editable as c (c.id)}
            {@const on = game.collections.includes(c.id)}
            <label class="flex h-8 cursor-pointer items-center gap-2.5 rounded-[4px] px-1.5 text-[13px] hover:bg-base-200">
              <input type="checkbox" class="checkbox checkbox-xs" checked={on} disabled={app.busy} onchange={() => app.toggleMembership(game.appid, c.id)} />
              <span class="spine" style="background:{c.color ?? NO_COLOR};opacity:{c.color ? 1 : 0.35}"></span>
              <span class="truncate {on ? 'font-medium' : 'text-muted'}">{c.name}</span>
            </label>
          {/each}
        </div>
      {:else}
        <p class="text-[13px] text-muted">You have no collections yet. Make one with the + next to Collections.</p>
      {/if}
    </section>

    <dl class="divide-y divide-base-300 border-y border-base-300 px-5 text-[13px]">
      {@render fact('Played', fmtHours(game.playtime_minutes))}
      {@render fact('Last played', fmtAgo(game.last_played))}
      {@render fact('Reviews', game.review_percent ? `${game.review_percent}% positive` : 'Unknown')}
      {#if game.metacritic}{@render fact('Metacritic', String(game.metacritic))}{/if}
      {@render fact('Steam Deck', DECK[game.extras.deck])}
      {@render fact('Runs on', platforms(game.extras.platforms))}
      {@render fact('On disk', game.installed_bytes ? fmtSize(game.installed_bytes) : game.download_bytes ? `Not installed (about ${fmtSize(game.download_bytes)})` : 'Not installed')}
      {@render fact('Screenshots', game.screenshots ? 'Yes' : 'None found')}
      {#if game.proton}{@render fact('ProtonDB', game.proton.charAt(0).toUpperCase() + game.proton.slice(1))}{/if}
    </dl>

    {#if game.store}
      <section class="border-b border-base-300 px-5 py-4 text-[13px]">
        <h3 class="mb-2 font-semibold">In the store ({app.library?.online.country})</h3>
        <div class="flex items-baseline gap-2">
          <span class="text-lg font-semibold tabular-nums">{game.store.price_text ?? 'Price unavailable'}</span>
          {#if game.store.discount}
            <span class="rounded-[4px] bg-success px-1.5 text-xs font-semibold leading-5 text-success-content">-{game.store.discount}%</span>
            {#if game.store.discount_ends}<span class="text-muted">until {new Date(game.store.discount_ends * 1000).toLocaleDateString(undefined, { month: 'short', day: 'numeric' })}</span>{/if}
          {/if}
        </div>
        {#if game.store.review_count}<p class="mt-1 text-muted">{game.store.review_label}, {game.store.review_count.toLocaleString()} reviews</p>{/if}
        <p class="mt-1 text-[11px] text-muted">Checked {fmtAgo(game.store.fetched).toLowerCase()}</p>
      </section>
    {/if}

    <section class="px-5 py-4">
      <h3 class="mb-2 flex items-center gap-1.5 text-[13px] font-semibold"><Icon name="trophy" size={14} /> Achievements</h3>
      {#if ach && ach.total > 0}
        <div class="flex items-baseline justify-between text-[13px]">
          <span class="text-muted">{ach.unlocked === ach.total ? 'All unlocked' : 'Unlocked'}</span>
          <span class="font-medium tabular-nums">{ach.unlocked} of {ach.total}</span>
        </div>
        <div class="mt-1.5 h-1.5 overflow-hidden rounded-full bg-base-300">
          <div class="h-full rounded-full bg-base-content" style="width:{pct(ach.unlocked, ach.total)}%"></div>
        </div>
      {:else if hunt}
        <p class="text-[13px] text-muted">{hunt.achievements} achievements. {app.settings.online_stats && app.settings.online_achievements ? 'Your progress is read from your Steam profile once you’ve played it.' : 'Turn on achievement progress in Settings to see yours.'}</p>
      {:else if !game.categories.includes(ACHIEVEMENTS_FEATURE)}
        <p class="text-[13px] text-muted">This game has no achievements.</p>
      {:else}
        <p class="text-[13px] text-muted">Steam hasn’t saved achievement progress for this game on this computer.</p>
      {/if}

      {#if hunt}
        <dl class="mt-3 text-[13px]">
          {@render fact('Median time to 100%', fmtSpan(hunt.median_minutes))}
          {@render fact('Fastest', fmtSpan(hunt.fastest_minutes))}
          {@render fact('Players with every achievement', `${hunt.perfected.toLocaleString()} (${pct(hunt.perfected, hunt.players)}%)`)}
        </dl>
        <p class="mt-1 text-[11px] text-muted">From SteamHunters</p>
      {/if}
    </section>

    {#if features.length}
      <section class="border-t border-base-300 px-5 py-4">
        <h3 class="mb-2 text-[13px] font-semibold">Features</h3>
        <div class="flex flex-wrap gap-1.5">
          {#each features as f (f.id)}
            <button class="h-6 rounded-field border px-2 text-xs {app.flagFilter.includes(f.id) ? 'border-primary bg-primary text-primary-content' : 'border-base-300 hover:border-base-content/30'}" onclick={() => app.toggleFlag(f.id)} title="Show only games with this feature">{f.name}</button>
          {/each}
        </div>
      </section>
    {/if}

    {#if topTags.length}
      <section class="border-t border-base-300 px-5 py-4">
        <h3 class="mb-2 text-[13px] font-semibold">Genres and tags</h3>
        <div class="flex flex-wrap gap-1.5">
          {#each topTags as t (t)}
            <button class="h-6 rounded-field border px-2 text-xs {app.tagFilter.includes(t) ? 'border-primary bg-primary text-primary-content' : 'border-base-300 hover:border-base-content/30'}" onclick={() => app.toggleTag(t)} title="Show only games with this tag">{app.tagNames[t] ?? t}</button>
          {/each}
        </div>
      </section>
    {/if}
  {:else}
    <div class="p-5 pt-1">
      <p class="font-display text-4xl font-semibold tabular-nums leading-none">{app.selected.length.toLocaleString()}</p>
      <p class="mt-1 text-muted">games selected</p>

      <h3 class="mb-2 mt-6 text-[13px] font-semibold">Add them to</h3>
      <div class="flex gap-2">
        <select class="select select-sm min-w-0 flex-1" bind:value={target} aria-label="Collection">
          <option value="" disabled>Choose a collection</option>
          {#each editable as c (c.id)}<option value={c.id}>{c.name}</option>{/each}
        </select>
        <button class="btn btn-primary btn-sm" disabled={!target || app.busy} onclick={addMany}>Add</button>
      </div>
      {#if app.isCollectionView}
        <button class="btn btn-sm mt-3 w-full text-error" disabled={app.busy} onclick={() => app.removeFromCollection(app.selected, app.view)}>Take them out of {app.viewTitle}</button>
      {/if}
      <button class="btn btn-sm mt-2 w-full" onclick={() => app.openNewCollection()}>New collection from these…</button>
      <button class="btn btn-ghost btn-sm mt-2 w-full" onclick={() => app.clearSelection()}>Clear selection</button>
    </div>
  {/if}
</aside>
{/if}
