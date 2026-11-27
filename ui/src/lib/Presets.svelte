<script lang="ts">
  // Ready-made sets of collections that fill themselves. Picking one makes the collections
  // that are missing and gives criteria to ones you already have by the same name.
  import * as api from './api';
  import { describeRule } from './describe';
  import { app } from './store.svelte';
  import Icon from './Icon.svelte';

  let presets = $state<api.Preset[]>([]);
  let chosen = $state<string | null>(null);
  let working = $state(false);

  const open = $derived(app.dialog === 'presets');
  $effect(() => {
    if (open && !presets.length) api.presets().then((p) => (presets = p)).catch((e) => app.fail(e));
  });

  const have = (name: string) => app.collections.some((c) => !c.dynamic && c.name.trim().toLowerCase() === name.toLowerCase());
  const preset = $derived(presets.find((p) => p.id === chosen) ?? null);

  async function add() {
    if (!preset) return;
    working = true;
    const ok = await app.applyPreset(preset.id);
    working = false;
    if (ok) close();
  }
  function close() {
    app.dialog = null;
    chosen = null;
  }
</script>

{#if open}
  <div class="modal modal-open" role="dialog" aria-label="Presets" tabindex="-1" onkeydown={(e) => e.key === 'Escape' && close()}>
    <div class="modal-box flex max-h-[calc(100vh-3rem)] max-w-xl flex-col rounded-box p-0">
      <header class="px-6 pb-2 pt-5">
        <h2 class="font-display text-xl font-semibold">Start from a preset</h2>
        <p class="mt-1 text-[13px] text-muted">A set of collections that fill themselves and let go of games that stop fitting. Collections you already have by the same name are reused, never duplicated.</p>
      </header>

      <div class="min-h-0 flex-1 overflow-y-auto px-6 pb-4 text-[13px]">
        <div class="flex flex-col divide-y divide-base-300 rounded-box border border-base-300" role="radiogroup" aria-label="Presets">
          {#each presets as p (p.id)}
            <label class="flex cursor-pointer items-start gap-3 p-3.5 {chosen === p.id ? 'bg-base-200' : 'hover:bg-base-200/60'}">
              <input type="radio" name="preset" class="radio radio-xs mt-0.5" checked={chosen === p.id} onchange={() => (chosen = p.id)} />
              <span class="min-w-0 flex-1">
                <strong class="block font-semibold">{p.name}</strong>
                <span class="text-muted">{p.description}</span>
                {#if chosen === p.id}
                  <ul class="mt-2.5 flex flex-col gap-1.5">
                    {#each p.collections as c (c.name)}
                      <li class="flex gap-2">
                        <span class="mt-1.5 size-1.5 shrink-0 rounded-full bg-base-content/40"></span>
                        <span><strong class="font-medium">{c.name}</strong>{#if have(c.name)} <span class="text-muted">(you have this one)</span>{/if}<span class="block text-xs text-muted">{describeRule(c.rule)}</span></span>
                      </li>
                    {/each}
                  </ul>
                {:else}
                  <span class="mt-1 block text-xs text-muted">{p.collections.map((c) => c.name).join(', ')}</span>
                {/if}
              </span>
            </label>
          {/each}
        </div>
        <p class="mt-3 text-muted">Like every change, nothing goes to Steam until you save, and Undo takes it back.</p>
      </div>

      <footer class="flex items-center justify-end gap-2 border-t border-base-300 px-6 py-3">
        <button class="btn btn-ghost btn-sm" onclick={close}>Cancel</button>
        <button class="btn btn-primary btn-sm" disabled={!preset || working} onclick={add}>
          {#if working}<span class="loading loading-spinner loading-xs"></span>{:else}<Icon name="plus" size={14} />{/if}
          {preset ? `Add ${preset.name}` : 'Choose a preset'}
        </button>
      </footer>
    </div>
    <button class="modal-backdrop" aria-label="Close" onclick={close}></button>
  </div>
{/if}
