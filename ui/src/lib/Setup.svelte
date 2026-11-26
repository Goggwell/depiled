<script lang="ts">
  import { onMount } from 'svelte';
  import * as api from './api';
  import { app } from './store.svelte';

  onMount(() => {
    // Keep checking, so the app moves on by itself once Steam is installed and signed in.
    const t = setInterval(async () => {
      app.status = await api.status();
      if (app.status.steam_found) {
        clearInterval(t);
        await app.refresh();
      }
    }, 3000);
    return () => clearInterval(t);
  });
</script>

<main class="grid h-full place-items-center p-6">
  <div class="max-w-md">
    <p class="font-display text-[22px] font-bold">Depiled</p>
    <h1 class="mt-6 font-display text-2xl font-semibold">Steam isn’t set up on this computer yet</h1>
    <p class="mt-2 text-muted">Depiled reads your library from Steam’s own files. Install Steam and sign in once, and this page continues by itself.</p>
    <div class="mt-5 flex items-center gap-2.5 text-[13px] text-muted"><span class="loading loading-ring loading-sm"></span> Looking for Steam</div>
  </div>
</main>
