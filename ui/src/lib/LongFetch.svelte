<script lang="ts">
  // Said once, when a refresh is going to take a while (usually a big library's first one):
  // roughly how long, that the app works meanwhile, and that stopping loses nothing.
  import { app } from './store.svelte';

  const plan = $derived(app.longFetch);
  const minutes = $derived(plan ? Math.max(5, Math.round(plan.seconds / 300) * 5) : 0);
  const time = $derived(minutes >= 90 ? `about ${Math.round(minutes / 60)} hours` : minutes >= 60 ? 'about an hour' : `about ${minutes} minutes`);

  function stop() {
    app.stopOnline();
    app.longFetch = null;
  }
</script>

{#if plan}
  <div class="modal modal-open z-[1100]" role="dialog" aria-label="This will take a while" tabindex="-1" onkeydown={(e) => e.key === 'Escape' && (app.longFetch = null)}>
    <div class="modal-box max-w-md rounded-box p-0 text-[13px]">
      <header class="px-6 pb-1 pt-5"><h2 class="font-display text-xl font-semibold">This will take a while</h2></header>
      <div class="px-6 pt-1">
        <p>
          Depiled is fetching details for {plan.games.toLocaleString()} games, {time} at the pace the sites allow. A big library’s first fetch is the long one; later ones only ask about what changed.
        </p>
        <ul class="mt-3 flex flex-col gap-1.5 text-muted">
          <li>Everything already on your computer works now. Details fill in as they arrive, recently played games first.</li>
          <li>If a site asks Depiled to slow down, it waits and carries on.</li>
          <li>You can stop it any time from the status bar. What arrived is kept, and the next fetch carries on from there.</li>
        </ul>
      </div>
      <footer class="mt-5 flex justify-end gap-2 border-t border-base-300 px-6 py-3">
        <button class="btn btn-ghost btn-sm" onclick={stop}>Stop for now</button>
        <!-- svelte-ignore a11y_autofocus -->
        <button class="btn btn-primary btn-sm" autofocus onclick={() => (app.longFetch = null)}>Keep going</button>
      </footer>
    </div>
  </div>
{/if}
