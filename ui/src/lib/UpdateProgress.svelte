<script lang="ts">
  import { app } from './store.svelte';

  let { wide = false }: { wide?: boolean } = $props();
  const u = $derived(app.update);
  const pct = $derived(u?.total ? Math.min(100, Math.floor((u.downloaded / u.total) * 100)) : null);
  const mb = (b: number) => `${(b / 1e6).toFixed(1)} MB`;
</script>

<!-- Download progress for an update: a bar and how far it is, then the install. -->
{#if u && u.phase !== 'ready'}
  <span class="flex min-w-0 items-center gap-2 text-muted" role="status">
    <progress class="progress h-1.5 {wide ? 'w-40' : 'w-24'}" value={u.phase === 'installing' ? undefined : u.downloaded} max={u.total ?? undefined} aria-label="Update download"></progress>
    <span class="truncate tabular-nums">
      {#if u.phase === 'installing'}Installing {u.info.version}, Depiled will restart…{:else if pct !== null}Downloading {u.info.version}, {pct}%{wide ? ` of ${mb(u.total ?? 0)}` : ""}{:else}Downloading {u.info.version}, {mb(u.downloaded)}{/if}
    </span>
  </span>
{/if}
