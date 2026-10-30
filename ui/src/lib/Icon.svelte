<script lang="ts">
  // Every icon in the app comes from Phosphor. Names here are what the screens ask for, so a
  // different icon can be swapped in one place.
  import type { Component } from 'svelte';
  import ArrowCounterClockwise from 'phosphor-svelte/lib/ArrowCounterClockwise';
  import Bell from 'phosphor-svelte/lib/Bell';
  import Books from 'phosphor-svelte/lib/Books';
  import Check from 'phosphor-svelte/lib/Check';
  import Clock from 'phosphor-svelte/lib/Clock';
  import ClockCounterClockwise from 'phosphor-svelte/lib/ClockCounterClockwise';
  import DotsThree from 'phosphor-svelte/lib/DotsThree';
  import Folder from 'phosphor-svelte/lib/Folder';
  import Funnel from 'phosphor-svelte/lib/Funnel';
  import Gear from 'phosphor-svelte/lib/Gear';
  import ListBullets from 'phosphor-svelte/lib/ListBullets';
  import Lock from 'phosphor-svelte/lib/Lock';
  import MagicWand from 'phosphor-svelte/lib/MagicWand';
  import MagnifyingGlass from 'phosphor-svelte/lib/MagnifyingGlass';
  import PencilSimple from 'phosphor-svelte/lib/PencilSimple';
  import Play from 'phosphor-svelte/lib/Play';
  import Plus from 'phosphor-svelte/lib/Plus';
  import SquaresFour from 'phosphor-svelte/lib/SquaresFour';
  import Trash from 'phosphor-svelte/lib/Trash';
  import Tray from 'phosphor-svelte/lib/Tray';
  import Trophy from 'phosphor-svelte/lib/Trophy';
  import X from 'phosphor-svelte/lib/X';

  type Weight = 'thin' | 'light' | 'regular' | 'bold' | 'fill' | 'duotone';

  let { name, size = 18, weight = 'regular' }: { name: string; size?: number; weight?: Weight } = $props();

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const icons: Record<string, Component<any>> = {
    search: MagnifyingGlass,
    undo: ArrowCounterClockwise,
    history: ClockCounterClockwise,
    wand: MagicWand,
    filter: Funnel,
    grid: SquaresFour,
    list: ListBullets,
    folder: Folder,
    library: Books,
    inbox: Tray,
    plus: Plus,
    close: X,
    check: Check,
    lock: Lock,
    dots: DotsThree,
    trophy: Trophy,
    play: Play,
    edit: PencilSimple,
    trash: Trash,
    gear: Gear,
    bell: Bell,
    clock: Clock,
  };

  const Glyph = $derived(icons[name]);
</script>

{#if Glyph}<Glyph {size} {weight} aria-hidden="true" />{/if}
