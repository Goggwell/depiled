<script lang="ts">
  import { app } from './store.svelte';
  import { describeRule } from './describe';
  import Icon from './Icon.svelte';

  const id = $derived(app.isCollectionView ? app.view : null);
  const collection = $derived(id ? app.collections.find((c) => c.id === id) : undefined);
  const rule = $derived(id ? app.smartRules[id] : undefined);
  const fresh = $derived(id ? app.newMatches(id).length : 0);
  const description = $derived(rule ? describeRule(rule) : '');
</script>

{#if collection && !collection.dynamic && rule}
  <div class="mx-6 mb-3 flex items-center gap-3 rounded-box border border-base-300 bg-base-100 py-2 pl-3.5 pr-2 text-[13px]">
    <Icon name="rules" size={16} />
    <span class="min-w-0 flex-1 truncate"><span class="text-muted">Fills itself with games that are</span> {description}</span>
    <button class="btn btn-ghost btn-sm" onclick={() => app.openRuleDialog(collection.id)}>Edit</button>
    <button class="btn btn-sm" disabled={fresh === 0 || app.busy} onclick={() => app.fillCollection(collection.id)}>
      {fresh === 0 ? 'Up to date' : `Add ${fresh.toLocaleString()} ${fresh === 1 ? 'match' : 'matches'}`}
    </button>
  </div>
{/if}
