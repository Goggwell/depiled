<script lang="ts">
  // The status bar's online indicator: what is being fetched right now, and a panel with each
  // source's last update. Fetching runs in the background, so the app stays usable throughout.
  import { app, fmtAgo } from './store.svelte';
  import Icon from './Icon.svelte';

  let open = $state(false);

  /** Which source each task the backend reports belongs to. */
  const TASK_SOURCE: Record<string, string> = {
    'achievement progress': 'online_achievements',
    'store details': 'online_prices',
    'prices for the new region': 'online_prices',
    'completion times': 'online_hunters',
    'ProtonDB ratings': 'online_protondb',
  };

  const sources = $derived(app.library?.online.sources ?? []);
  const running = $derived(app.online);
  const pct = $derived(running?.total ? Math.round((running.done / running.total) * 100) : 0);
  const newest = $derived(Math.max(0, ...sources.filter((s) => app.settings[s.key]).map((s) => s.updated)));
</script>

<svelte:window onclick={() => (open = false)} onkeydown={(e) => e.key === 'Escape' && (open = false)} />

{#if app.settings.online_stats}
  <div class="relative">
    <button
      class="flex h-7 items-center gap-2 rounded-field px-2 text-muted hover:bg-base-200 hover:text-base-content"
      aria-expanded={open}
      title="Details from the internet"
      onclick={(e) => {
        e.stopPropagation();
        open = !open;
      }}
    >
      {#if running}
        <span class="loading loading-spinner loading-xs"></span>
        <span class="max-w-56 truncate">Getting {running.task}{running.total ? ` ${running.done.toLocaleString()} of ${running.total.toLocaleString()}` : ''}</span>
        {#if running.total}<span class="h-1 w-16 overflow-hidden rounded-full bg-base-300"><span class="block h-full bg-base-content/60 transition-[width]" style="width:{pct}%"></span></span>{/if}
      {:else}
        <Icon name="timer" size={14} />
        <span>Online details {newest ? `updated ${fmtAgo(newest).toLowerCase()}` : 'not fetched yet'}</span>
      {/if}
    </button>

    {#if open}
      <div
        class="absolute bottom-9 right-0 z-40 w-80 rounded-box border border-base-300 bg-base-100 p-3 text-[13px] text-base-content shadow-xl"
        role="dialog"
        aria-label="Details from the internet"
        tabindex="-1"
        onclick={(e) => e.stopPropagation()}
        onkeydown={() => {}}
      >
        <p class="mb-2 font-semibold">Details from the internet</p>
        <ul class="flex flex-col gap-1.5">
          {#each sources as s (s.key)}
            {@const on = app.settings[s.key]}
            {@const busy = !!running && (running.active ?? [running.task]).some((t) => TASK_SOURCE[t] === s.key)}
            <li class="flex items-baseline justify-between gap-3 {on ? '' : 'opacity-50'}">
              <span>{s.label}</span>
              <span class="text-right text-xs tabular-nums text-muted">
                {#if !on}Off{:else if busy}Fetching…{:else if s.updated}{s.items.toLocaleString()} games, {fmtAgo(s.updated).toLowerCase()}{:else}Not yet{/if}
              </span>
            </li>
          {/each}
        </ul>
        <p class="mt-2 text-xs text-muted">Everything is kept on this computer, so it works offline. Only what's new or due is fetched.</p>
        <div class="mt-3 flex justify-end gap-2">
          <button class="btn btn-ghost btn-xs" onclick={() => ((open = false), (app.dialog = 'settings'))}>Settings</button>
          {#if running}
            <button class="btn btn-xs" onclick={() => app.stopOnline()}>Stop</button>
          {:else}
            <button class="btn btn-xs" onclick={() => app.refreshOnline()}>Check now</button>
          {/if}
        </div>
      </div>
    {/if}
  </div>
{/if}
