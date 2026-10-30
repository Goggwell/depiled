<script lang="ts">
  import { onMount } from 'svelte';
  import * as api from './api';
  import { app } from './store.svelte';
  import Icon from './Icon.svelte';

  let enabling = $state(false);
  let error = $state<string | null>(null);

  const s = $derived(app.status);

  onMount(() => {
    // Keep checking, so the app moves on by itself as soon as Steam is ready.
    const t = setInterval(async () => {
      app.status = await api.status();
      if (app.status.ready) {
        clearInterval(t);
        await app.refresh();
      }
    }, 2000);
    return () => clearInterval(t);
  });

  async function enable() {
    enabling = true;
    error = null;
    try {
      await api.enableSetup();
      app.status = await api.status();
    } catch (e) {
      error = String(e);
    } finally {
      enabling = false;
    }
  }
</script>

<main class="grid h-full place-items-center p-6">
  <div class="card w-full max-w-md border border-base-300 bg-base-100 shadow-2xl">
    <div class="card-body items-center gap-3 px-9 py-10 text-center">
      <span class="grid size-14 place-items-center rounded-box bg-primary text-primary-content"><Icon name="library" size={28} /></span>
      <h1 class="font-display text-2xl font-semibold">Let's connect to Steam</h1>

      {#if s && !s.steam_found}
        <p class="text-muted">We couldn't find Steam on this computer. Install and sign in to Steam, then reopen this app.</p>
      {:else if s && !s.flag_file}
        <p class="text-muted">
          To organize your collections, this app needs a one-time permission to talk to Steam on your computer. Nothing is sent online, and you can switch it off any time.
        </p>
        <button class="btn btn-primary btn-lg mt-2" disabled={enabling} onclick={enable}>
          {#if enabling}<span class="loading loading-spinner loading-sm"></span>{/if}
          {enabling ? 'Working…' : 'Allow it'}
        </button>
        {#if error}<p class="text-error">{error}</p>{/if}
      {:else if s && !s.steam_running}
        <p class="text-muted">Almost there. Start Steam and sign in, and this page will continue by itself.</p>
        <div class="mt-1 flex items-center gap-2.5 text-muted"><span class="loading loading-ring loading-sm"></span> Waiting for Steam…</div>
      {:else}
        <p class="text-muted">
          The permission is set. <strong class="text-base-content">Now fully close Steam and open it again</strong>, including the icon near the clock, so it can take effect. This page continues on its own when Steam is ready.
        </p>
        <div class="mt-1 flex items-center gap-2.5 text-muted"><span class="loading loading-ring loading-sm"></span> Waiting for Steam…</div>
      {/if}
    </div>
  </div>
</main>
