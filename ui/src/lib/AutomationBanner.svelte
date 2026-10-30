<script lang="ts">
  import { app } from './store.svelte';
  import Icon from './Icon.svelte';

  const summary = $derived(
    (app.pending ?? [])
      .slice(0, 3)
      .map((o) => `${o.count} ${o.kind === 'remove' ? 'out of' : 'into'} ${o.collection}`)
      .join(', '),
  );
</script>

{#if app.pending && app.pending.length}
  <div class="mx-5 mb-3 flex items-center gap-3 rounded-box border border-secondary/40 bg-base-100 py-2.5 pl-3.5 pr-3" role="status">
    <span class="grid size-8 shrink-0 place-items-center rounded-field bg-primary text-primary-content"><Icon name="bell" size={16} /></span>
    <div class="min-w-0 flex-1">
      <strong class="block text-sm">Your library has {app.pending.length} {app.pending.length === 1 ? 'change' : 'changes'} ready</strong>
      <span class="block truncate text-[13px] text-muted">{summary}{app.pending.length > 3 ? ', and more' : ''}</span>
    </div>
    <button class="btn btn-sm btn-ghost" onclick={() => (app.dialog = 'organize')}>Review</button>
    <button class="btn btn-sm btn-primary" disabled={app.busy} onclick={() => app.applyPending()}>Apply now</button>
    <button class="btn btn-sm btn-ghost btn-square" aria-label="Not now" onclick={() => (app.pending = null)}><Icon name="close" size={14} /></button>
  </div>
{:else if app.showAutomationPrompt}
  <div class="mx-5 mb-3 rounded-box border border-base-300 bg-base-100 p-4" role="status">
    <div class="flex items-start gap-3">
      <span class="grid size-8 shrink-0 place-items-center rounded-field bg-primary text-primary-content"><Icon name="wand" size={16} /></span>
      <div class="flex-1">
        <strong>Want me to keep your library tidy?</strong>
        <p class="mt-0.5 text-sm text-muted">When you start or finish a game, I can move it to the right collection. You can change this any time in Settings.</p>
        <div class="mt-3 flex flex-wrap gap-2">
          <button class="btn btn-sm btn-primary" onclick={() => app.setAutomation('notify')}>Tell me first</button>
          <button class="btn btn-sm" onclick={() => app.setAutomation('auto')}>Do it automatically</button>
          <button class="btn btn-sm btn-ghost" onclick={() => app.setAutomation('off')}>No thanks</button>
        </div>
      </div>
    </div>
  </div>
{/if}
