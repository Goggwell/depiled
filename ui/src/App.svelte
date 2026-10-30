<script lang="ts">
  import { onMount } from 'svelte';
  import { isDesktop } from './lib/api';
  import { app } from './lib/store.svelte';
  import AutomationBanner from './lib/AutomationBanner.svelte';
  import CollectionBar from './lib/CollectionBar.svelte';
  import Details from './lib/Details.svelte';
  import Dialogs from './lib/Dialogs.svelte';
  import Grid from './lib/Grid.svelte';
  import Icon from './lib/Icon.svelte';
  import NewCollection from './lib/NewCollection.svelte';
  import Settings from './lib/Settings.svelte';
  import Setup from './lib/Setup.svelte';
  import Sidebar from './lib/Sidebar.svelte';
  import Toolbar from './lib/Toolbar.svelte';

  onMount(() => {
    app.init().catch((e) => app.fail(e));
  });
</script>

{#if !app.status}
  <div class="grid h-full place-items-center text-muted"><span class="loading loading-ring loading-lg"></span></div>
{:else if !app.status.ready}
  <Setup />
{:else}
  <div class="flex h-full">
    <Sidebar />
    <section class="flex min-w-0 flex-1 flex-col">
      <Toolbar />
      <AutomationBanner />
      <CollectionBar />
      <Grid />
    </section>
    <Details />
  </div>
  <Dialogs />
  <NewCollection />
  <Settings />
{/if}

{#if !isDesktop}
  <div class="badge badge-warning fixed bottom-3 right-3 z-50 pointer-events-none font-semibold">Demo mode: invented games, nothing here touches Steam.</div>
{/if}

{#if app.toast}
  <div class="toast toast-center z-50">
    <div class="alert {app.toast.error ? 'alert-error' : 'alert-neutral'} shadow-xl" role="status">
      <span>{app.toast.text}</span>
      <div class="flex gap-1">
        {#if app.toast.canUndo && app.undoStack.length}
          <button class="btn btn-sm" onclick={() => app.undo()}>Undo</button>
        {/if}
        <button class="btn btn-ghost btn-sm btn-square" aria-label="Dismiss" onclick={() => (app.toast = null)}><Icon name="close" size={14} /></button>
      </div>
    </div>
  </div>
{/if}
