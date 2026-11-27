<script lang="ts">
  import { tick } from 'svelte';
  import { coverUrl } from './api';
  import type { AccountView } from './api';
  import { app } from './store.svelte';
  import Icon from './Icon.svelte';

  let open = $state(false);
  let button = $state<HTMLButtonElement>();
  let menu = $state<HTMLDivElement>();

  const list = $derived(app.accounts?.accounts ?? []);
  const current = $derived(app.currentAccount);
  const following = $derived(app.accounts?.picked == null);
  const signedIn = $derived(app.accounts?.signed_in ?? null);
  const label = (a: AccountView) => a.name || a.login || `Account ${a.id}`;

  async function toggle() {
    open = !open;
    if (open) {
      app.loadAccounts(); // unsaved counts may have changed
      await tick();
      menu?.querySelector<HTMLElement>('[role="menuitemradio"][aria-checked="true"]')?.focus();
    }
  }
  function close() {
    open = false;
    button?.focus();
  }
  function pick(id: number | null) {
    open = false;
    app.switchAccount(id);
  }
  function keys(e: KeyboardEvent) {
    const items = [...(menu?.querySelectorAll<HTMLElement>('[role^="menuitem"]') ?? [])];
    const at = items.indexOf(document.activeElement as HTMLElement);
    const go = (i: number) => items[(i + items.length) % items.length]?.focus();
    if (e.key === 'ArrowDown') go(at + 1);
    else if (e.key === 'ArrowUp') go(at - 1);
    else if (e.key === 'Escape' || e.key === 'Tab') close();
    else return;
    e.preventDefault();
    e.stopPropagation();
  }
</script>

{#snippet avatar(a: AccountView, size: number)}
  {@const src = coverUrl(a.avatar)}
  {#if src}
    <img {src} alt="" class="shrink-0 rounded-full object-cover" style="width:{size}px;height:{size}px" />
  {:else}
    <span class="grid shrink-0 place-items-center rounded-full bg-base-300 text-[11px] font-semibold text-base-content/70" style="width:{size}px;height:{size}px">{[...label(a)][0]?.toUpperCase()}</span>
  {/if}
{/snippet}

<svelte:window onclick={() => (open = false)} />

{#if current}
  <!-- Whose library this is. With more than one account on the computer it opens a list to
       switch; each account keeps its own unsaved changes, history and rules. -->
  <div class="relative border-t border-base-300 p-2">
    {#if list.length > 1}
      <button
        bind:this={button}
        class="flex w-full items-center gap-2.5 rounded-field p-1.5 text-left hover:bg-base-200 disabled:opacity-60"
        aria-haspopup="menu"
        aria-expanded={open}
        disabled={app.saving || app.busy}
        title="Switch Steam account"
        onclick={(e) => {
          e.stopPropagation();
          toggle();
        }}
      >
        {@render avatar(current, 28)}
        <span class="min-w-0 flex-1 leading-tight">
          <span class="block truncate font-semibold">{label(current)}</span>
          <span class="block truncate text-xs text-muted">{following ? 'Following Steam’s sign-in' : current.id === signedIn ? 'Signed in to Steam' : 'Chosen in Depiled'}</span>
        </span>
        <span class="text-muted"><Icon name="caret" size={13} /></span>
      </button>
    {:else}
      <div class="flex items-center gap-2.5 p-1.5">
        {@render avatar(current, 28)}
        <span class="min-w-0 flex-1 truncate font-semibold">{label(current)}</span>
      </div>
    {/if}

    {#if open}
      <div
        bind:this={menu}
        class="absolute bottom-full left-2 right-2 z-30 mb-1 rounded-box border border-base-300 bg-base-100 p-1.5 text-[13px] shadow-[0_8px_28px_rgb(15_20_30/0.2)]"
        role="menu"
        aria-label="Steam accounts"
        tabindex="-1"
        onclick={(e) => e.stopPropagation()}
        onkeydown={keys}
      >
        {#each list as a (a.id)}
          <button
            role="menuitemradio"
            aria-checked={a.id === current.id}
            class="flex w-full items-center gap-2.5 rounded-[4px] px-2 py-1.5 text-left hover:bg-base-200 focus-visible:bg-base-200 focus-visible:outline-none"
            onclick={() => pick(a.id)}
          >
            {@render avatar(a, 24)}
            <span class="min-w-0 flex-1 leading-tight">
              <span class="block truncate {a.id === current.id ? 'font-semibold' : ''}">{label(a)}</span>
              <span class="block truncate text-xs text-muted">{a.id === signedIn ? 'Signed in to Steam' : a.login}</span>
            </span>
            {#if a.pending}
              <span class="shrink-0 rounded-[3px] bg-accent px-1.5 text-xs font-bold leading-5 text-accent-content tabular-nums" title="{a.pending} {a.pending === 1 ? 'change' : 'changes'} not in Steam yet">{a.pending}</span>
            {/if}
            {#if a.id === current.id}<span class="shrink-0"><Icon name="check" size={14} /></span>{/if}
          </button>
        {/each}
        <div class="my-1 border-t border-base-300"></div>
        <button
          role="menuitemradio"
          aria-checked={following}
          class="flex w-full items-start gap-2.5 rounded-[4px] px-2 py-1.5 text-left hover:bg-base-200 focus-visible:bg-base-200 focus-visible:outline-none"
          onclick={() => pick(null)}
        >
          <span class="mt-0.5 w-6 shrink-0 text-center">{#if following}<Icon name="check" size={14} />{/if}</span>
          <span class="leading-tight">Follow Steam’s sign-in<span class="block text-xs text-muted">Show whoever is signed in to Steam, and switch when they change.</span></span>
        </button>
      </div>
    {/if}
  </div>
{/if}
