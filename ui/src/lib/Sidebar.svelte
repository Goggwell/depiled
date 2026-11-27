<script lang="ts">
  import { tick } from 'svelte';
  import type { Collection } from './api';
  import { app, COLORS, NO_COLOR, OTHER_KINDS } from './store.svelte';
  import AccountSwitcher from './AccountSwitcher.svelte';
  import EmojiPicker from './EmojiPicker.svelte';
  import OrderToggle from './OrderToggle.svelte';
  import Icon from './Icon.svelte';

  let dropTarget = $state<string | null>(null);
  // Reordering by dragging, when the user keeps their own order: which collection is being
  // dragged, and where the line shows (before a collection, or at the end).
  let dragging = $state<string | null>(null);
  let insertAt = $state<{ before: string | null } | null>(null);
  const userCollections = $derived(app.collections.filter((c) => !c.dynamic));

  function reorderOver(e: DragEvent, id: string) {
    if (!dragging || !e.dataTransfer?.types.includes('application/x-collection')) return false;
    e.preventDefault();
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const lower = e.clientY > r.top + r.height / 2;
    const i = userCollections.findIndex((c) => c.id === id);
    insertAt = { before: lower ? (userCollections[i + 1]?.id ?? null) : id };
    return true;
  }
  function reorderDrop(e: DragEvent) {
    if (!dragging || !insertAt) return false;
    e.preventDefault();
    const { before } = insertAt;
    const id = dragging;
    dragging = null;
    insertAt = null;
    if (before !== id) app.moveCollection(id, before);
    return true;
  }
  let renaming = $state<string | null>(null);
  let renameValue = $state('');
  let renameInput = $state<HTMLInputElement>();
  let renameBox = $state<HTMLSpanElement>();
  let confirmDelete = $state<string | null>(null);
  /** Collection whose menu is open, and where. */
  let menu = $state<{ id: string; x: number; y: number } | null>(null);

  const total = $derived(app.gameCount);
  const menuFor = $derived(menu ? app.collections.find((c) => c.id === menu!.id) : undefined);
  /** A colour picked with the colour wheel rather than from the swatches. */
  const customColor = $derived(menuFor?.color && !COLORS.some((c) => c.value === menuFor.color) ? menuFor.color : null);

  function over(e: DragEvent, id: string) {
    if (!e.dataTransfer?.types.includes('application/x-games')) return;
    e.preventDefault();
    e.dataTransfer.dropEffect = e.shiftKey ? 'move' : 'copy';
    dropTarget = id;
  }

  function drop(e: DragEvent, id: string) {
    dropTarget = null;
    const raw = e.dataTransfer?.getData('application/x-games');
    if (!raw) return;
    e.preventDefault();
    app.addToCollection(JSON.parse(raw) as number[], id, e.shiftKey);
  }

  async function commitRename() {
    const id = renaming;
    const name = renameValue.trim();
    renaming = null;
    if (id && name && name !== app.collections.find((c) => c.id === id)?.name) await app.renameCollection(id, name);
  }

  function openMenu(e: MouseEvent, c: Collection) {
    if (c.dynamic) return;
    e.preventDefault();
    opener = e.currentTarget as HTMLElement;
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    menu = { id: c.id, x: e.type === 'contextmenu' ? e.clientX : rect.right - 8, y: e.type === 'contextmenu' ? e.clientY : rect.bottom + 4 };
    confirmDelete = null;
  }

  /** Runs a menu action, then closes the menu. The action runs first: it needs the collection,
   *  which is looked up from the open menu. */
  function act(f: () => void) {
    f();
    closeMenu();
  }

  // The menu takes focus when it opens, arrow keys move through it, and Esc gives focus back
  // to the button that opened it.
  let menuEl = $state<HTMLDivElement>();
  // Opens where it was asked for, then moves up as far as it needs to stay inside the window.
  let menuTop = $state(0);
  $effect(() => {
    if (!menu) return;
    menuTop = menu.y;
    if (menuEl) menuTop = Math.max(8, Math.min(menu.y, window.innerHeight - menuEl.offsetHeight - 8));
  });
  let opener: HTMLElement | null = null;
  $effect(() => {
    if (menu && menuEl) menuEl.querySelector<HTMLElement>('[role="menuitem"]')?.focus();
  });
  function closeMenu() {
    menu = null;
    opener?.focus();
    opener = null;
  }
  function menuKeys(e: KeyboardEvent) {
    const items = [...(menuEl?.querySelectorAll<HTMLElement>('[role="menuitem"]') ?? [])];
    const at = items.indexOf(document.activeElement as HTMLElement);
    const go = (i: number) => items[(i + items.length) % items.length]?.focus();
    if (e.key === 'ArrowDown' || e.key === 'ArrowRight') go(at + 1);
    else if (e.key === 'ArrowUp' || e.key === 'ArrowLeft') go(at - 1);
    else if (e.key === 'Home') go(0);
    else if (e.key === 'End') go(items.length - 1);
    else if (e.key === 'Escape' || e.key === 'Tab') closeMenu();
    else return;
    e.preventDefault();
    e.stopPropagation();
  }

  const focus = (node: HTMLInputElement) => {
    node.focus();
    node.select();
  };

  const row = 'group relative flex h-8 w-full items-center gap-2.5 rounded-field pl-2 pr-1.5 text-left transition-colors';
  const active = (on: boolean) => (on ? 'bg-base-200 font-semibold' : 'hover:bg-base-200/70');
</script>

<svelte:window onclick={() => (menu = null)} onkeydown={(e) => e.key === 'Escape' && menu && closeMenu()} />

<aside class="flex w-60 shrink-0 flex-col border-r border-base-300 bg-base-100" aria-label="Library and collections">
  <div class="px-4 pb-3 pt-4">
    <span class="font-display text-[22px] font-extrabold leading-none tracking-[-0.03em]">Depiled</span>
  </div>

  <nav class="flex min-h-0 flex-1 flex-col gap-px overflow-y-auto px-2 pb-2">
    <button class="{row} {active(app.view === 'all')}" onclick={() => (app.view = 'all')}>
      <Icon name="library" size={16} /> <span class="flex-1 truncate">All games</span>
      <span class="text-xs tabular-nums text-muted">{total.toLocaleString()}</span>
    </button>
    <button class="{row} {active(app.view === 'none')}" onclick={() => (app.view = 'none')}>
      <Icon name="inbox" size={16} /> <span class="flex-1 truncate">Not in a collection</span>
      <span class="text-xs tabular-nums text-muted">{app.uncategorizedCount.toLocaleString()}</span>
    </button>
    {#each OTHER_KINDS.filter((k) => app.kindCounts[k.kind]) as k (k.kind)}
      <button class="{row} {active(app.view === `kind:${k.kind}`)} text-muted" onclick={() => (app.view = `kind:${k.kind}`)}>
        <span class="w-4"></span> <span class="flex-1 truncate">{k.label}</span>
        <span class="text-xs tabular-nums">{app.kindCounts[k.kind]?.toLocaleString()}</span>
      </button>
    {/each}

    <div class="mt-4 flex items-center justify-between pb-1 pl-2 pr-1">
      <span class="text-xs font-semibold text-muted">Collections</span>
      <span class="flex">
        <OrderToggle />
        <button class="btn btn-ghost btn-xs btn-square" aria-label="Start from a preset" title="Start from a preset" onclick={() => (app.dialog = 'presets')}><Icon name="wand" size={14} /></button>
        <button class="btn btn-ghost btn-xs btn-square" aria-label="New collection" title="New collection" onclick={() => app.openNewCollection()}><Icon name="plus" size={14} /></button>
      </span>
    </div>

    {#each app.collections as c (c.id)}
      {#if insertAt && insertAt.before === c.id}<div class="mx-2 h-0.5 rounded-full bg-base-content" aria-hidden="true"></div>{/if}
      <div
        class="{row} {active(app.view === c.id)} {dropTarget === c.id ? 'bg-base-200 ring-2 ring-inset' : ''} {dragging === c.id ? 'opacity-50' : ''}"
        style={dropTarget === c.id ? `--tw-ring-color:${c.color ?? 'var(--color-base-content)'}` : ''}
        role="listitem"
        draggable={app.ordered && !c.dynamic && renaming !== c.id}
        ondragstart={(e) => {
          if (!app.ordered || c.dynamic) return;
          dragging = c.id;
          e.dataTransfer?.setData('application/x-collection', c.id);
          if (e.dataTransfer) e.dataTransfer.effectAllowed = 'move';
        }}
        ondragend={() => ((dragging = null), (insertAt = null))}
        ondragover={(e) => !c.dynamic && !reorderOver(e, c.id) && over(e, c.id)}
        ondragleave={() => (dropTarget = null)}
        ondrop={(e) => !c.dynamic && !reorderDrop(e) && drop(e, c.id)}
        oncontextmenu={(e) => openMenu(e, c)}
      >
        <span class="spine" style="background:{c.color ?? NO_COLOR};opacity:{c.color ? 1 : 0.35}"></span>
        {#if renaming === c.id}
          <!-- Picking an emoji moves focus into the picker; that isn't the end of renaming. -->
          <span class="flex min-w-0 flex-1 items-center" bind:this={renameBox}>
          <input
            type="text"
            class="input input-xs h-6 min-w-0 flex-1"
            bind:value={renameValue}
            bind:this={renameInput}
            use:focus
            onblur={(e) => !renameBox?.contains(e.relatedTarget as Node) && !(e.relatedTarget as Element | null)?.closest?.('[aria-label="Emoji"]') && commitRename()}
            onkeydown={(e) => {
              if (e.key === 'Enter') commitRename();
              if (e.key === 'Escape') renaming = null;
            }}
          />
          <EmojiPicker target={renameInput} onpick={(v) => (renameValue = v)} ondismiss={commitRename} />
          </span>
        {:else}
          <button
            class="flex h-full min-w-0 flex-1 items-center gap-1.5 text-left"
            title={c.dynamic ? 'Managed by Steam, so it can’t be changed here' : ''}
            onclick={() => (app.view = c.id)}
            ondblclick={() => !c.dynamic && ((renaming = c.id), (renameValue = c.name))}
          >
            <span class="truncate">{c.name}</span>
            {#if c.dynamic}<span class="shrink-0 text-muted"><Icon name="lock" size={12} /></span>{/if}
          </button>
          {#if app.changedSet.has(c.id)}<span class="size-1.5 shrink-0 rounded-full bg-accent" title="Has changes that aren’t in Steam yet"></span>{/if}
          <span class="w-8 text-right text-xs tabular-nums text-muted {c.dynamic ? '' : 'group-focus-within:invisible group-hover:invisible'}">{c.count.toLocaleString()}</span>
          {#if !c.dynamic}
            <!-- Shown on hover, and whenever it has keyboard focus, so Tab can reach it. -->
            <button class="btn btn-ghost btn-xs btn-square absolute right-1 opacity-0 focus-visible:opacity-100 group-hover:opacity-100 {menu?.id === c.id ? 'opacity-100' : ''}" aria-label="More for {c.name}" aria-haspopup="menu" aria-expanded={menu?.id === c.id} onclick={(e) => { e.stopPropagation(); openMenu(e, c); }}><Icon name="dots" size={16} weight="bold" /></button>
          {/if}
        {/if}
      </div>
    {/each}
    {#if insertAt && insertAt.before === null}<div class="mx-2 h-0.5 rounded-full bg-base-content" aria-hidden="true"></div>{/if}
  </nav>

  <div class="border-t border-base-300 p-3 text-xs leading-relaxed text-muted">Drag games onto a collection to add them. Hold Shift to move them out of the one you’re in.</div>
  <AccountSwitcher />
</aside>

{#if menu && menuFor}
  <div
    bind:this={menuEl}
    class="fixed z-50 max-h-[calc(100vh-16px)] w-64 overflow-y-auto rounded-box border border-base-300 bg-base-100 p-1.5 text-sm shadow-[0_8px_28px_rgb(15_20_30/0.22)]"
    style="left:{Math.min(menu.x, window.innerWidth - 270)}px;top:{menuTop}px"
    role="menu"
    aria-label="{menuFor.name} options"
    tabindex="-1"
    onclick={(e) => e.stopPropagation()}
    onkeydown={menuKeys}
  >
    <p class="px-2 pb-1 pt-1 text-xs text-muted">Colour</p>
    <!-- The colour in use has a tick in it; nothing is drawn around it, so swatches never
         overlap. The last two are any colour (a colour wheel) and no colour. Picking a colour
         keeps the menu open, so colours can be tried one after another. -->
    <div class="grid grid-cols-6 gap-1.5 px-2 pb-2" role="group" aria-label="Colour">
      {#each COLORS as col (col.value)}
        <button
          role="menuitem"
          class="grid size-7 place-items-center rounded-[5px] text-white transition-transform hover:scale-110 focus-visible:scale-110 focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-base-content"
          style="background:{col.value}"
          aria-label={col.name}
          aria-current={menuFor.color === col.value}
          title={col.name}
          onclick={() => app.setColor(menuFor.id, col.value)}
        >{#if menuFor.color === col.value}<span class="drop-shadow-[0_1px_1px_rgb(0_0_0/0.5)]"><Icon name="check" size={14} weight="bold" /></span>{/if}</button>
      {/each}
      <!-- The colour wheel: a rainbow ring, with your own colour inside once you've picked one,
           so it never looks like one of the swatches. It changes as you drag. -->
      <label
        class="relative grid size-7 cursor-pointer place-items-center rounded-[5px] p-[3px] text-white transition-transform hover:scale-110 focus-within:outline-2 focus-within:outline-offset-1 focus-within:outline-base-content"
        style="background:conic-gradient(#e5484d, #d6409f, #8e4ec6, #0090ff, #12a594, #86b31a, #ad7f58, #e5484d)"
        title={customColor ? `Your colour, ${customColor}. Click to change it.` : 'Any colour'}
      >
        <span class="grid size-full place-items-center rounded-[3px] {customColor ? '' : 'bg-base-100/80 text-base-content'}" style={customColor ? `background:${customColor}` : ''}>
          {#if customColor}<span class="drop-shadow-[0_1px_1px_rgb(0_0_0/0.5)]"><Icon name="check" size={13} weight="bold" /></span>{:else}<Icon name="plus" size={12} />{/if}
        </span>
        <input
          type="color"
          role="menuitem"
          class="absolute inset-0 size-full cursor-pointer opacity-0"
          aria-label="Any colour"
          value={customColor ?? '#5b5bd6'}
          oninput={(e) => app.liveColor(menuFor.id, e.currentTarget.value)}
        />
      </label>
      <button role="menuitem" class="grid size-7 place-items-center rounded-[5px] border border-base-300 text-muted transition-transform hover:scale-110 hover:text-base-content" aria-label="No colour" aria-current={!menuFor.color} title="No colour" onclick={() => app.setColor(menuFor.id, null)}>
        <Icon name={menuFor.color ? 'close' : 'check'} size={13} weight={menuFor.color ? 'regular' : 'bold'} />
      </button>
    </div>
    <div class="my-1 border-t border-base-300"></div>
    {#each [
      ['edit', 'Rename', () => ((renaming = menuFor.id), (renameValue = menuFor.name))],
      ['copy', 'Duplicate', () => app.duplicateCollection(menuFor.id)],
      ['merge', 'Merge others into this…', () => app.openMerge(menuFor.id)],
      ['rules', 'Auto-fill…', () => app.openRuleDialog(menuFor.id)],
      ...(app.ordered
        ? [
            ['up', 'Move up', () => app.nudgeCollection(menuFor.id, -1)],
            ['down', 'Move down', () => app.nudgeCollection(menuFor.id, 1)],
          ]
        : []),
    ] as [icon, label, run] (label)}
      <button role="menuitem" class="flex w-full items-center gap-2.5 rounded-[4px] px-2 py-1.5 text-left hover:bg-base-200 focus-visible:bg-base-200 focus-visible:outline-none" onclick={() => act(run as () => void)}><Icon name={icon as string} size={15} /> {label}</button>
    {/each}
    <div class="my-1 border-t border-base-300"></div>
    {#if confirmDelete === menuFor.id}
      <div class="flex flex-col gap-2 px-2 py-1.5">
        <span>Delete {menuFor.name}? Its {menuFor.count.toLocaleString()} {menuFor.count === 1 ? 'game stays' : 'games stay'} in your library, and Undo brings the collection back.</span>
        <span class="flex justify-end gap-1.5">
          <button role="menuitem" class="btn btn-ghost btn-xs" onclick={() => (confirmDelete = null)}>Keep it</button>
          <button role="menuitem" class="btn btn-error btn-xs" onclick={() => act(() => app.deleteCollection(menuFor.id))}>Delete</button>
        </span>
      </div>
    {:else}
      <button role="menuitem" class="flex w-full items-center gap-2.5 rounded-[4px] px-2 py-1.5 text-left text-error hover:bg-base-200 focus-visible:bg-base-200 focus-visible:outline-none" onclick={async () => { confirmDelete = menuFor.id; await tick(); menuEl?.querySelector<HTMLElement>('.btn-error')?.focus(); }}><Icon name="trash" size={15} /> Delete…</button>
    {/if}
  </div>
{/if}
