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
  import StatusBar from './lib/StatusBar.svelte';
  import Toolbar from './lib/Toolbar.svelte';

  onMount(() => {
    app.init().catch((e) => app.fail(e));
  });
</script>

{#if !app.status}
  <div class="grid h-full place-items-center text-muted"><span class="loading loading-ring loading-lg"></span></div>
{:else if !app.status.steam_found}
  <Setup />
{:else}
  <!-- The status bar spans the window, so the details panel (an overlay on narrow windows)
       never covers the save button. -->
  <div class="flex h-full flex-col">
    <div class="relative flex min-h-0 flex-1">
      <Sidebar />
      <main class="flex min-w-0 flex-1 flex-col bg-base-200">
        <Toolbar />
        <AutomationBanner />
        <CollectionBar />
        <Grid />
      </main>
      <Details />
    </div>
    <StatusBar />
  </div>
  <Dialogs />
  <NewCollection />
  <Settings />
{/if}

{#if !isDesktop}
  <div class="pointer-events-none fixed left-[118px] top-[19px] z-50 rounded-[4px] bg-neutral px-2.5 py-1 text-xs font-semibold text-neutral-content" title="A demo with invented games">Demo</div>
{/if}

{#if app.toast}
  <div class="toast toast-center z-50 mb-12">
    <div class="flex items-center gap-3 rounded-box py-2 pl-4 pr-2 text-sm shadow-lg {app.toast.error ? 'bg-error text-error-content' : 'bg-neutral text-neutral-content'}" role="status">
      {#if app.toast.error}<Icon name="warning" size={16} />{/if}
      <span class="max-w-md">{app.toast.text}</span>
      {#if app.toast.undo && app.draft.undo}
        <button class="btn btn-sm btn-ghost text-inherit" onclick={() => app.undo()}>Undo</button>
      {/if}
      <button class="btn btn-ghost btn-sm btn-square text-inherit" aria-label="Dismiss" onclick={() => (app.toast = null)}><Icon name="close" size={14} /></button>
    </div>
  </div>
{/if}
