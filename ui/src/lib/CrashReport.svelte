<script lang="ts">
  // Asks whether to send a crash log. Shown on its own in the small window that opens when
  // Depiled crashes (\`standalone\`), or over the library the next time Depiled starts if that
  // window never got the chance.
  import * as api from './api';
  import { app } from './store.svelte';
  import Icon from './Icon.svelte';

  let { crash, standalone = false }: { crash: api.Crash; standalone?: boolean } = $props();

  let note = $state('');
  let email = $state('');
  let showLog = $state(false);
  let sending = $state(false);
  let sent = $state(false);
  let problem = $state<string | null>(null);
  let copied = $state(false);

  const when = $derived(new Date(crash.at * 1000).toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'short' }));
  const report = (): api.Report => ({ kind: 'crash', message: note.trim() || '(no description)', email: email.trim() || null, details: crash.text });

  async function send() {
    sending = true;
    problem = null;
    try {
      await api.sendReport(report());
      await api.settleCrash(crash.id).catch(() => {});
      sent = true;
    } catch (e) {
      problem = String(e instanceof Error ? e.message : e);
    } finally {
      sending = false;
    }
  }

  async function dontSend() {
    await api.settleCrash(crash.id).catch(() => {});
    finish();
  }

  async function copy() {
    copied = await api.copyText(await api.reportText(report()));
  }

  /** Back to the library: restart it from the crash window, or just close the popup. */
  function finish() {
    if (standalone) api.startNormally();
    else app.crash = null;
  }
</script>

<div class="flex flex-col {standalone ? 'h-full' : 'max-h-[calc(100vh-3rem)]'} text-[13px]">
  <header class="flex items-start gap-3 px-6 pb-2 pt-6">
    <span class="mt-0.5 text-error"><Icon name="warning" size={22} weight="fill" /></span>
    <div>
      <h2 class="font-display text-xl font-semibold">Depiled closed unexpectedly</h2>
      <p class="mt-0.5 text-muted">{when}</p>
    </div>
  </header>

  {#if sent}
    <div class="flex-1 px-6 pt-2">
      <p class="flex items-center gap-2"><span class="text-success"><Icon name="done" size={18} weight="fill" /></span> Report sent. Thank you, this is what gets crashes fixed.</p>
      <p class="mt-2 text-muted">Changes you hadn’t saved to Steam were kept on this computer and will be there when Depiled opens.</p>
    </div>
    <footer class="flex justify-end gap-2 border-t border-base-300 px-6 py-3">
      <button class="btn btn-primary btn-sm" onclick={finish}>{standalone ? 'Open Depiled' : 'Done'}</button>
    </footer>
  {:else}
    <div class="min-h-0 flex-1 overflow-y-auto px-6 pt-2">
      <p>Sending a report tells the developer what went wrong. It contains the error, where in the program it happened, and your Windows and Depiled versions. Nothing about your Steam account or games.</p>
      <p class="mt-2 text-muted">Changes you hadn’t saved to Steam are kept, and will be there when Depiled opens again.</p>

      <label class="mt-4 block" for="crash-note">What were you doing just before? <span class="text-muted">(optional)</span></label>
      <textarea id="crash-note" class="textarea textarea-sm mt-1.5 h-24 w-full resize-y text-[13px] leading-relaxed" bind:value={note}></textarea>

      <label class="mt-3 block" for="crash-email">Your email, if you’d like to hear back <span class="text-muted">(optional)</span></label>
      <input id="crash-email" type="email" class="input input-sm mt-1.5 w-full" bind:value={email} autocomplete="email" />

      <button class="link mt-3 text-muted hover:text-base-content" onclick={() => (showLog = !showLog)}>{showLog ? 'Hide' : 'Show'} the crash log</button>
      {#if showLog}
        <pre class="mt-2 max-h-56 overflow-auto whitespace-pre-wrap rounded-field bg-base-200 p-2.5 text-xs leading-relaxed">{crash.text}</pre>
      {/if}

      {#if problem}
        <p class="mt-4 rounded-field bg-base-200 p-2.5" role="alert">Couldn’t send: {problem}. You can copy the report and send it another way.</p>
      {/if}
      {#if !app.canSendReports}
        <p class="mt-4 text-muted">This copy of Depiled can’t send reports itself. Copy the report and paste it into an issue on Depiled’s GitHub page.</p>
      {/if}
    </div>

    <footer class="mt-4 flex items-center gap-2 border-t border-base-300 px-6 py-3">
      <button class="btn btn-ghost btn-sm" onclick={copy}>{copied ? 'Copied' : 'Copy report'}</button>
      <span class="flex-1"></span>
      <button class="btn btn-ghost btn-sm" onclick={dontSend}>Don’t send</button>
      {#if app.canSendReports}
        <button class="btn btn-primary btn-sm" onclick={send} disabled={sending}>{#if sending}<span class="loading loading-spinner loading-xs"></span> Sending…{:else}Send report{/if}</button>
      {/if}
    </footer>
  {/if}
</div>
