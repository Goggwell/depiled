<script lang="ts">
  import type { Automation } from './api';
  import { app } from './store.svelte';
  import type { Theme } from './store.svelte';
  import Icon from './Icon.svelte';

  const modes: { value: Automation; title: string; text: string }[] = [
    { value: 'off', title: 'Never', text: 'I will press Auto-organize myself.' },
    { value: 'notify', title: 'Tell me', text: 'When you launch or finish a game, I will offer the changes and you decide.' },
    { value: 'auto', title: 'Do it for me', text: 'Apply the changes by myself. Every change is saved first, so Undo always works.' },
  ];
  const themes: [Theme, string][] = [
    ['system', 'Match my system'],
    ['light', 'Light'],
    ['dark', 'Dark'],
  ];

  const close = () => (app.dialog = null);
</script>

{#if app.dialog === 'settings'}
  <div class="modal modal-open" role="dialog" aria-label="Settings" tabindex="-1" onkeydown={(e) => e.key === 'Escape' && close()}>
    <div class="modal-box flex max-h-[calc(100vh-3rem)] max-w-xl flex-col p-0">
      <header class="px-6 pb-1 pt-5"><h2 class="font-display text-xl font-semibold">Settings</h2></header>

      <div class="min-h-0 flex-1 overflow-y-auto px-6 py-3">
        <h3 class="mb-1 text-xs font-bold uppercase tracking-wider text-muted">Keeping your library tidy</h3>
        <p class="mb-3 text-sm text-muted">While this app is open, it can notice when you start or finish a game and apply your rules.</p>
        <div class="flex flex-col gap-2" role="radiogroup" aria-label="Automatic organizing">
          {#each modes as m (m.value)}
            <label class="flex cursor-pointer items-start gap-3 rounded-box border p-3.5 transition-colors {app.settings.automation === m.value ? 'border-secondary bg-base-200' : 'border-base-300 hover:bg-base-200/60'}">
              <input type="radio" name="automation" class="radio radio-primary radio-sm mt-1" checked={app.settings.automation === m.value} onchange={() => app.setAutomation(m.value)} />
              <span class="flex flex-col"><strong>{m.title}</strong><span class="text-sm text-muted">{m.text}</span></span>
            </label>
          {/each}
        </div>

        <h3 class="mb-2 mt-7 text-xs font-bold uppercase tracking-wider text-muted">Appearance</h3>
        <div class="join">
          {#each themes as [v, label]}
            <button class="btn join-item btn-sm {app.theme === v ? 'btn-primary' : ''}" onclick={() => app.setTheme(v)}>{label}</button>
          {/each}
        </div>

        <div class="mt-7 flex items-start gap-3 rounded-box bg-base-200 p-3.5 text-sm text-muted">
          <Icon name="lock" size={16} />
          <span>Everything stays on this computer. The app only talks to Steam on your machine and never goes online.</span>
        </div>
      </div>

      <footer class="flex justify-end border-t border-base-300 px-6 py-3.5"><button class="btn btn-primary" onclick={close}>Done</button></footer>
    </div>
    <button class="modal-backdrop" aria-label="Close" onclick={close}></button>
  </div>
{/if}
