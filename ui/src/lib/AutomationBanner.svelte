<script lang="ts">
  import { app } from './store.svelte';
  import Icon from './Icon.svelte';

  const summary = $derived(
    (app.pending ?? [])
      .slice(0, 3)
      .map((o) => `${o.count} ${o.kind === 'remove' ? 'out of' : 'into'} ${o.collection}`)
      .join(', '),
  );
  const box = 'mx-6 mb-3 rounded-box border border-base-300 bg-base-100 text-[13px]';
</script>

{#if app.pending && app.pending.length}
  <div class="{box} flex items-center gap-3 py-2 pl-3.5 pr-2" role="status">
    <Icon name="bell" size={16} />
    <div class="min-w-0 flex-1">
      <strong class="font-semibold">Your rules have {app.pending.length} {app.pending.length === 1 ? 'change' : 'changes'} ready.</strong>
      <span class="text-muted">{summary}{app.pending.length > 3 ? ', and more' : ''}</span>
    </div>
    <button class="btn btn-ghost btn-sm" onclick={() => (app.dialog = 'organize')}>Review</button>
    <button class="btn btn-sm" disabled={app.busy} onclick={() => app.applyPending()}>Make the changes</button>
    <button class="btn btn-ghost btn-sm btn-square" aria-label="Not now" onclick={() => (app.pending = null)}><Icon name="close" size={14} /></button>
  </div>
{:else if app.showAutomationPrompt || app.showStatsPrompt}
  <!-- First run: the two choices that change how the app behaves, side by side, once. Both
       can be changed later in Settings. -->
  <div class="{box} grid gap-x-6 gap-y-4 p-4 md:grid-cols-2" role="region" aria-label="Two choices to start">
    <div class={app.settings.automation !== null ? 'opacity-60' : ''}>
      <strong class="font-semibold">Sort games as you play?</strong>
      <p class="mt-0.5 text-muted">When you start, stop or finish a game, it can move to Started, Unfinished or Completed.</p>
      <div class="seg mt-2.5" role="group" aria-label="Sort games as you play">
        {#each [['notify', 'Ask me first'], ['auto', 'Do it for me'], ['off', 'No']] as [v, label]}
          <button aria-pressed={app.settings.automation === v} onclick={() => app.setAutomation(v as 'notify' | 'auto' | 'off')}>{label}</button>
        {/each}
      </div>
    </div>
    <div class={app.settings.online_stats !== null ? 'opacity-60' : ''}>
      <strong class="font-semibold">Add details from the internet?</strong>
      <p class="mt-0.5 text-muted">Your achievement progress, prices and sales, and completion times. Kept on this computer, so it all still works offline. No account needed.</p>
      <div class="seg mt-2.5" role="group" aria-label="Add details from the internet">
        <button aria-pressed={app.settings.online_stats === true} onclick={() => app.updateSettings({ online_stats: true })}>Yes, add them</button>
        <button aria-pressed={app.settings.online_stats === false} onclick={() => app.updateSettings({ online_stats: false })}>Stay offline</button>
      </div>
    </div>
  </div>
{/if}
