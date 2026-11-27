<script lang="ts">
  import { app } from './store.svelte';
  import Icon from './Icon.svelte';
  import OnlineStatus from './OnlineStatus.svelte';
  import UpdateProgress from './UpdateProgress.svelte';

  const n = $derived(app.draft.pending);
  const savedAt = $derived(app.lastSaved ? new Date(app.lastSaved.at).toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' }) : '');
</script>

<!-- Where the draft lives: an amber edge and an amber save button while Steam doesn't have the
     changes yet. Amber appears nowhere else in the app. -->
<footer
  class="flex h-11 shrink-0 items-center gap-3 border-t-2 bg-base-100 px-4 text-[13px] {n ? 'border-accent' : 'border-base-300'}"
  aria-live="polite"
>
  {#if app.saving}
    <span class="loading loading-spinner loading-xs"></span>
    <span>{app.savingClosesSteam ? 'Closing Steam, writing your collections, then opening Steam again…' : 'Writing your collections to Steam…'}</span>
  {:else if !app.library}
    <span class="text-muted">Reading your library…</span>
  {:else if n}
    <span class="flex min-w-0 items-center gap-1.5">
      <span class="rounded-[3px] bg-accent px-1.5 text-xs font-bold leading-5 text-accent-content tabular-nums">{n}</span>
      <span class="truncate"><strong class="font-semibold">{n === 1 ? 'change' : 'changes'}</strong> not in Steam yet</span>
    </span>
    {#if app.draft.rebased}<span class="truncate text-muted">Steam changed meanwhile; both are kept.</span>{/if}
  {:else if app.lastSaved}
    <span class="text-success"><Icon name="done" size={15} weight="fill" /></span>
    <span class="truncate">Saved {app.lastSaved.changes} {app.lastSaved.changes === 1 ? 'change' : 'changes'} to Steam at {savedAt}.</span>
    <button class="link text-muted hover:text-base-content" onclick={() => (app.dialog = 'history')}>History</button>
  {:else}
    <span class="text-muted"><Icon name="done" size={15} /></span>
    <span class="text-muted">Steam has all your changes</span>
  {/if}

  <span class="flex-1"></span>

  {#if app.update?.phase === 'ready'}
    <button class="btn btn-ghost btn-sm gap-1.5 font-normal" disabled={app.saving} onclick={() => app.installUpdate()} title="Downloads it and restarts Depiled. Changes that aren’t in Steam yet are kept.">
      <Icon name="download" size={15} /> Update to {app.update.info.version}
    </button>
  {:else if app.update}
    <UpdateProgress />
  {/if}

  <OnlineStatus />

  <span class="flex shrink-0 items-center gap-1.5 text-muted" title={app.draft.steam_running ? 'Steam has to close while saving' : 'Steam is closed'}>
    <span class="size-1.5 rounded-full {app.draft.steam_running ? 'bg-success' : 'bg-base-content/30'}"></span>
    <span class="max-lg:hidden">Steam {app.draft.steam_running ? 'open' : 'closed'}</span>
  </span>

  {#if n && !app.saving}
    <button class="btn btn-ghost btn-sm" disabled={app.busy} onclick={() => app.discard()} title="Throw away the changes that aren’t in Steam yet. Undo brings them back.">Discard</button>
    <button class="btn btn-accent btn-sm" disabled={app.busy} onclick={() => (app.dialog = 'save')}><Icon name="save" size={15} /> Review and save</button>
  {/if}
</footer>
