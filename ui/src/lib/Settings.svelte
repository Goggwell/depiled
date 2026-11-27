<script lang="ts">
  import type { Automation } from './api';
  import { app } from './store.svelte';
  import type { Theme } from './store.svelte';
  import Icon from './Icon.svelte';
  import UpdateProgress from './UpdateProgress.svelte';

  const modes: { value: Automation; title: string; text: string }[] = [
    { value: 'off', title: 'Never', text: 'I’ll press Auto-organize myself.' },
    { value: 'notify', title: 'Ask me', text: 'When you start or finish a game, Depiled offers the changes and you decide.' },
    { value: 'auto', title: 'Do it for me', text: 'Depiled makes the changes and saves them the next time Steam is closed. Every save can be undone from History.' },
  ];
  const themes: [Theme, string][] = [
    ['system', 'Match my system'],
    ['light', 'Light'],
    ['dark', 'Dark'],
  ];

  const close = () => (app.dialog = null);

  type SourceKey = 'online_achievements' | 'online_prices' | 'online_protondb' | 'online_hunters';
  const sources: { key: SourceKey; title: string; text: string }[] = [
    { key: 'online_achievements', title: 'Achievement progress and screenshots', text: 'Read from your public Steam profile for each game you’ve played: again after you play it, when an update adds achievements, and every six to eight months. Which games you have screenshots of is checked weekly.' },
    { key: 'online_prices', title: 'Prices, sales and review counts', text: 'From the Steam store for your region, checked weekly.' },
    { key: 'online_protondb', title: 'ProtonDB ratings', text: 'How well each game runs on Linux and Steam Deck. New ratings are checked about monthly. Established ratings are checked less often.' },
    { key: 'online_hunters', title: 'Completion times', text: 'Time to unlock every achievement, from SteamHunters. Checked when Steam reports that a game changed, and monthly otherwise. Also how Depiled notices that an update added achievements to a game you finished.' },
  ];
  const mb = (bytes: number) => bytes === 0 ? '0 KB' : (bytes >= 1e6 ? `${(bytes / 1e6).toFixed(1)} MB` : `${Math.max(1, Math.round(bytes / 1e3))} KB`);
  const usage = $derived(app.library?.online.usage);
  const saved = $derived(new Set(app.library?.online.cached_regions ?? []));
  // Common Steam store regions; anything else can be set by following this computer's region.
  const regions: [string, string][] = [
    ['AR', 'Argentina'], ['AU', 'Australia'], ['BR', 'Brazil'], ['CA', 'Canada'], ['CL', 'Chile'], ['CN', 'China'], ['CO', 'Colombia'],
    ['DE', 'Germany'], ['ES', 'Spain'], ['FR', 'France'], ['GB', 'United Kingdom'], ['HK', 'Hong Kong'], ['ID', 'Indonesia'], ['IN', 'India'],
    ['IT', 'Italy'], ['JP', 'Japan'], ['KR', 'South Korea'], ['KZ', 'Kazakhstan'], ['MX', 'Mexico'], ['MY', 'Malaysia'], ['NL', 'Netherlands'],
    ['NO', 'Norway'], ['NZ', 'New Zealand'], ['PE', 'Peru'], ['PH', 'Philippines'], ['PL', 'Poland'], ['RU', 'Russia'], ['SA', 'Saudi Arabia'],
    ['SE', 'Sweden'], ['SG', 'Singapore'], ['TH', 'Thailand'], ['TR', 'Türkiye'], ['TW', 'Taiwan'], ['UA', 'Ukraine'], ['US', 'United States'],
    ['VN', 'Vietnam'], ['ZA', 'South Africa'],
  ];
  const section = 'border-t border-base-300 py-5 first:border-t-0 first:pt-1';
</script>

{#if app.dialog === 'settings'}
  <div class="modal modal-open" role="dialog" aria-label="Settings" tabindex="-1" onkeydown={(e) => e.key === 'Escape' && close()}>
    <div class="modal-box flex max-h-[calc(100vh-3rem)] max-w-xl flex-col rounded-box p-0">
      <header class="px-6 pb-2 pt-5"><h2 class="font-display text-xl font-semibold">Settings</h2></header>

      <div class="min-h-0 flex-1 overflow-y-auto px-6 text-[13px]">
        <section class={section}>
          <h3 class="font-semibold">Organize as I play</h3>
          <p class="mb-3 mt-0.5 text-muted">While Depiled is open, it notices when you play and checks your rules.</p>
          <div class="flex flex-col divide-y divide-base-300 rounded-box border border-base-300" role="radiogroup" aria-label="Organize as I play">
            {#each modes as m (m.value)}
              <label class="flex cursor-pointer items-start gap-3 p-3 {app.settings.automation === m.value ? 'bg-base-200' : 'hover:bg-base-200/60'}">
                <input type="radio" name="automation" class="radio radio-xs mt-0.5" checked={app.settings.automation === m.value} onchange={() => app.setAutomation(m.value)} />
                <span><strong class="block font-semibold">{m.title}</strong><span class="text-muted">{m.text}</span></span>
              </label>
            {/each}
          </div>
        </section>

        <section class={section}>
          <h3 class="font-semibold">Saving to Steam</h3>
          <label class="mt-2 flex cursor-pointer items-center gap-2.5">
            <input type="checkbox" class="checkbox checkbox-xs" checked={app.settings.reopen_steam} onchange={(e) => app.updateSettings({ reopen_steam: e.currentTarget.checked })} />
            Open Steam again after closing it to save
          </label>
        </section>

        <section class={section}>
          <h3 class="font-semibold">Details from the internet</h3>
          <label class="mt-2 flex cursor-pointer items-start gap-2.5">
            <input type="checkbox" class="checkbox checkbox-xs mt-0.5" checked={!!app.settings.online_stats} onchange={(e) => app.updateSettings({ online_stats: e.currentTarget.checked })} />
            <span>Fill in details from the internet<span class="block text-muted">Fetched in the background now and then and kept on this computer, so everything still works offline. No account or key needed.</span></span>
          </label>
          {#if app.settings.online_stats}
            <div class="ml-6 mt-3 flex flex-col gap-2.5">
              {#each sources as s (s.key)}
                <label class="flex cursor-pointer items-start gap-2.5">
                  <input type="checkbox" class="checkbox checkbox-xs mt-0.5" checked={app.settings[s.key]} onchange={(e) => app.updateSettings({ [s.key]: e.currentTarget.checked })} />
                  <span>{s.title}<span class="block text-muted">{s.text}</span></span>
                </label>
              {/each}
              {#if app.library?.online.profile_private && app.settings.online_achievements}
                <p class="rounded-field bg-base-200 p-2.5">Your Steam profile’s game details are private, so achievement progress can’t be read. In Steam, open your profile, choose Edit Profile, then Privacy Settings, and set Game details to Public.</p>
              {/if}

              <p class="text-muted">The first download depends on your library size. Later checks reuse saved data. Achievement pages and ProtonDB ratings are requested one game at a time.</p>

              <div class="mt-1 flex items-center gap-3">
                <label for="region" class="flex-1">Store region for prices<span class="block text-muted">Saved regional prices are reused. Missing or outdated prices are fetched in the background.</span></label>
                <select id="region" class="select select-sm w-56" value={app.settings.region ?? ''} onchange={(e) => app.updateSettings({ region: e.currentTarget.value || null })}>
                  <option value="">This computer’s region ({app.library?.online.detected_region ?? 'US'})</option>
                  {#each regions as [code, name]}<option value={code}>{name} ({code}){saved.has(code) ? ', prices saved' : ''}</option>{/each}
                </select>
              </div>

              <div class="mt-1 flex items-center gap-3">
                <span class="flex-1 text-muted">
                  {#if app.online}Getting {app.online.task}…{:else}{app.huntersKnown.toLocaleString()} games have completion times, {(app.library?.online.achievements_read ?? 0).toLocaleString()} have achievement progress from your profile.{#if usage?.month} Estimated download {mb(usage.month_bytes)} this month ({usage.month_requests.toLocaleString()} requests).{/if}{/if}
                </span>
                <button class="btn btn-sm" disabled={!!app.online} onclick={() => app.refreshOnline(true)}>Fetch everything now</button>
              </div>
            </div>
          {/if}
        </section>
        <section class={section}>
          <h3 class="mb-2 font-semibold">Appearance</h3>
          <div class="seg" role="group" aria-label="Appearance">
            {#each themes as [v, label]}
              <button aria-pressed={app.theme === v} onclick={() => app.setTheme(v)}>{label}</button>
            {/each}
          </div>
        </section>

        <section class={section}>
          <h3 class="font-semibold">Updates</h3>
          <p class="mt-0.5 text-muted">{app.version ? `This is Depiled ${app.version}.` : ''} Updates are downloaded from Depiled’s GitHub page and installed in place. Changes that aren’t in Steam yet are kept.</p>
          <label class="mt-3 flex cursor-pointer items-center gap-2.5">
            <input type="checkbox" class="checkbox checkbox-xs" checked={app.settings.check_updates} onchange={(e) => app.updateSettings({ check_updates: e.currentTarget.checked })} />
            Check for a new version when Depiled starts
          </label>
          <div class="mt-3 flex items-start gap-3">
            <span class="flex-1">Get<span class="block text-muted">{app.settings.update_channel === 'nightly' ? 'The newest build, made after every change. Gets fixes first, and is tested less.' : 'Numbered releases only.'}</span></span>
            <div class="seg" role="group" aria-label="Which versions to get">
              <button aria-pressed={app.settings.update_channel === 'stable'} onclick={() => app.updateSettings({ update_channel: 'stable' })}>Releases</button>
              <button aria-pressed={app.settings.update_channel === 'nightly'} onclick={() => app.updateSettings({ update_channel: 'nightly' })}>Nightly builds</button>
            </div>
          </div>
          <div class="mt-3 flex min-h-8 items-center gap-3">
            {#if app.update?.phase === 'ready'}
              <span class="flex-1"><strong class="font-semibold">Depiled {app.update.info.version}</strong> is available.</span>
              <button class="btn btn-primary btn-sm" disabled={app.saving} onclick={() => app.installUpdate()}><Icon name="download" size={15} /> Update and restart</button>
            {:else if app.update}
              <span class="flex-1"><UpdateProgress wide /></span>
            {:else}
              <span class="flex-1 text-muted">{app.checkingUpdate ? 'Checking…' : ''}</span>
              <button class="btn btn-sm" disabled={app.checkingUpdate} onclick={() => app.checkUpdate(true)}>Check now</button>
            {/if}
          </div>
          {#if app.update?.info.notes}
            <p class="mt-2 max-h-32 overflow-y-auto whitespace-pre-line rounded-field bg-base-200 p-2.5 text-muted">{app.update.info.notes}</p>
          {/if}
        </section>

        {#if app.status?.legacy_debug_port}
          <section class={section}>
            <h3 class="font-semibold">Steam’s debug port is still on</h3>
            <p class="mt-0.5 text-muted">An older version of Depiled switched it on. Depiled no longer needs it, and leaving it on lets other programs on this computer control Steam.</p>
            <button class="btn btn-sm mt-2" onclick={() => app.disableDebugPort()}>Switch it off</button>
          </section>
        {/if}

        <section class="{section} text-muted">
          Your library is read from Steam’s files on this computer. With details from the internet on, Depiled asks Steam, SteamHunters and ProtonDB about your games and reads your public profile. Checking for updates asks GitHub for the newest version number. Feedback and crash reports are only sent when you press Send. Nothing else leaves this computer.
        </section>
      </div>

      <footer class="flex justify-end border-t border-base-300 px-6 py-3"><button class="btn btn-primary btn-sm" onclick={close}>Done</button></footer>
    </div>
    <button class="modal-backdrop" aria-label="Close" onclick={close}></button>
  </div>
{/if}
