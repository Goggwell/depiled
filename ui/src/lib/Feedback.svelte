<script lang="ts">
  import { tick, untrack } from 'svelte';
  import * as api from './api';
  import { app } from './store.svelte';
  import Icon from './Icon.svelte';

  type Kind = 'bug' | 'idea' | 'other';
  const kinds: [Kind, string][] = [
    ['bug', 'Something’s wrong'],
    ['idea', 'An idea'],
    ['other', 'Something else'],
  ];
  const prompts: Record<Kind, string> = {
    bug: 'What happened, and what did you expect? Steps to make it happen again help most.',
    idea: 'What would you like Depiled to do?',
    other: 'What’s on your mind?',
  };

  let kind = $state<Kind>('bug');
  let message = $state('');
  let email = $state('');
  let includeDetails = $state(true);
  let details = $state('');
  let showDetails = $state(false);
  let sending = $state(false);
  let sent = $state(false);
  let problem = $state<string | null>(null);
  let copied = $state(false);
  let box = $state<HTMLTextAreaElement>();

  const open = $derived(app.dialog === 'feedback');

  // Fresh each time it opens, starting from what the app was given (an error's Report button).
  $effect(() => {
    if (!open) return;
    untrack(reset);
  });
  function reset() {
    kind = 'bug';
    message = app.feedbackSeed ?? '';
    email = '';
    includeDetails = true;
    showDetails = false;
    sent = false;
    problem = null;
    copied = false;
    api.reportDetails(app.recentErrors).then((d) => (details = d)).catch(() => (details = ''));
    // Ready to type, after any text the dialog started with.
    tick().then(() => {
      box?.focus();
      box?.setSelectionRange(message.length, message.length);
    });
  }

  const report = (): api.Report => ({ kind, message, email: email.trim() || null, details: includeDetails && details ? details : null });

  async function send() {
    sending = true;
    problem = null;
    try {
      await api.sendReport(report());
      sent = true;
    } catch (e) {
      problem = String(e instanceof Error ? e.message : e);
    } finally {
      sending = false;
    }
  }

  async function copy() {
    copied = await api.copyText(await api.reportText(report()));
  }

  function close() {
    app.dialog = null;
    app.feedbackSeed = null;
  }
</script>

{#if open}
  <div class="modal modal-open" role="dialog" aria-label="Send feedback" tabindex="-1" onkeydown={(e) => e.key === 'Escape' && close()}>
    <div class="modal-box flex max-h-[calc(100vh-3rem)] max-w-lg flex-col rounded-box p-0">
      <header class="px-6 pb-1 pt-5"><h2 class="font-display text-xl font-semibold">Send feedback</h2></header>

      {#if sent}
        <div class="px-6 pb-2 pt-2 text-[13px]">
          <p class="flex items-center gap-2"><span class="text-success"><Icon name="done" size={18} weight="fill" /></span> Sent. Thank you, it goes straight to the developer.</p>
          {#if email.trim()}<p class="mt-2 text-muted">Any reply will go to {email.trim()}.</p>{/if}
        </div>
        <footer class="flex justify-end px-6 py-4"><button class="btn btn-primary btn-sm" onclick={close}>Done</button></footer>
      {:else}
        <div class="min-h-0 flex-1 overflow-y-auto px-6 pt-2 text-[13px]">
          <div class="seg" role="group" aria-label="What kind of feedback">
            {#each kinds as [k, label] (k)}
              <button aria-pressed={kind === k} onclick={() => (kind = k)}>{label}</button>
            {/each}
          </div>

          <label class="mt-4 block" for="feedback-message">{prompts[kind]}</label>
          <textarea id="feedback-message" class="textarea textarea-sm mt-1.5 h-36 w-full resize-y text-[13px] leading-relaxed" bind:value={message} bind:this={box}></textarea>

          <label class="mt-3 block" for="feedback-email">Your email, if you’d like a reply <span class="text-muted">(optional)</span></label>
          <input id="feedback-email" type="email" class="input input-sm mt-1.5 w-full" bind:value={email} autocomplete="email" />

          <label class="mt-4 flex cursor-pointer items-start gap-2.5">
            <input type="checkbox" class="checkbox checkbox-xs mt-0.5" bind:checked={includeDetails} />
            <span>Include technical details<span class="block text-muted">Versions, Windows version, how many games and collections you have, a few settings and recent error messages. No names, Steam account or library contents.</span></span>
          </label>
          {#if includeDetails}
            <button class="link mt-1.5 pl-6 text-muted hover:text-base-content" onclick={() => (showDetails = !showDetails)}>{showDetails ? 'Hide' : 'Show'} exactly what’s sent</button>
            {#if showDetails}
              <pre class="ml-6 mt-2 max-h-48 overflow-auto whitespace-pre-wrap rounded-field bg-base-200 p-2.5 text-xs leading-relaxed">{details || 'Gathering…'}</pre>
            {/if}
          {/if}

          {#if problem}
            <p class="mt-4 rounded-field bg-base-200 p-2.5" role="alert">Couldn’t send: {problem}. You can copy the report and send it another way.</p>
          {/if}
          {#if !app.canSendReports}
            <p class="mt-4 text-muted">This copy of Depiled can’t send reports itself. Copy the report and paste it into an issue on Depiled’s GitHub page.</p>
          {/if}
        </div>

        <footer class="mt-4 flex items-center gap-2 border-t border-base-300 px-6 py-3">
          <button class="btn btn-ghost btn-sm" onclick={copy} disabled={!message.trim()}>{copied ? 'Copied' : 'Copy report'}</button>
          <span class="flex-1"></span>
          <button class="btn btn-ghost btn-sm" onclick={close}>Cancel</button>
          {#if app.canSendReports}
            <button class="btn btn-primary btn-sm" onclick={send} disabled={sending || !message.trim()}>{#if sending}<span class="loading loading-spinner loading-xs"></span> Sending…{:else}Send{/if}</button>
          {/if}
        </footer>
      {/if}
    </div>
    <button class="modal-backdrop" aria-label="Close" onclick={close}></button>
  </div>
{/if}
