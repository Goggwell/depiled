<script lang="ts">
  // Every icon in the app comes from Phosphor. Names here are what the screens ask for, so a
  // different icon can be swapped in one place.
  import type { Component } from 'svelte';
  import ArrowClockwise from 'phosphor-svelte/lib/ArrowClockwise';
  import ArrowCounterClockwise from 'phosphor-svelte/lib/ArrowCounterClockwise';
  import ArrowsMerge from 'phosphor-svelte/lib/ArrowsMerge';
  import Bell from 'phosphor-svelte/lib/Bell';
  import Books from 'phosphor-svelte/lib/Books';
  import CaretDown from 'phosphor-svelte/lib/CaretDown';
  import Check from 'phosphor-svelte/lib/Check';
  import CheckCircle from 'phosphor-svelte/lib/CheckCircle';
  import Clock from 'phosphor-svelte/lib/Clock';
  import ClockCounterClockwise from 'phosphor-svelte/lib/ClockCounterClockwise';
  import Copy from 'phosphor-svelte/lib/Copy';
  import DotsThree from 'phosphor-svelte/lib/DotsThree';
  import FloppyDisk from 'phosphor-svelte/lib/FloppyDisk';
  import Funnel from 'phosphor-svelte/lib/Funnel';
  import GameController from 'phosphor-svelte/lib/GameController';
  import Gear from 'phosphor-svelte/lib/Gear';
  import Heart from 'phosphor-svelte/lib/Heart';
  import UsersThree from 'phosphor-svelte/lib/UsersThree';
  import ListBullets from 'phosphor-svelte/lib/ListBullets';
  import Lock from 'phosphor-svelte/lib/Lock';
  import MagicWand from 'phosphor-svelte/lib/MagicWand';
  import MagnifyingGlass from 'phosphor-svelte/lib/MagnifyingGlass';
  import Palette from 'phosphor-svelte/lib/Palette';
  import PencilSimple from 'phosphor-svelte/lib/PencilSimple';
  import Plus from 'phosphor-svelte/lib/Plus';
  import SquaresFour from 'phosphor-svelte/lib/SquaresFour';
  import GridNine from 'phosphor-svelte/lib/GridNine';
  import Keyboard from 'phosphor-svelte/lib/Keyboard';
  import ListChecks from 'phosphor-svelte/lib/ListChecks';
  import Timer from 'phosphor-svelte/lib/Timer';
  import Trash from 'phosphor-svelte/lib/Trash';
  import Tray from 'phosphor-svelte/lib/Tray';
  import Trophy from 'phosphor-svelte/lib/Trophy';
  import Users from 'phosphor-svelte/lib/Users';
  import Warning from 'phosphor-svelte/lib/Warning';
  import X from 'phosphor-svelte/lib/X';

  type Weight = 'thin' | 'light' | 'regular' | 'bold' | 'fill' | 'duotone';

  let { name, size = 18, weight = 'regular' }: { name: string; size?: number; weight?: Weight } = $props();

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const icons: Record<string, Component<any>> = {
    search: MagnifyingGlass,
    undo: ArrowCounterClockwise,
    redo: ArrowClockwise,
    history: ClockCounterClockwise,
    wand: MagicWand,
    filter: Funnel,
    grid: SquaresFour,
    gridSmall: GridNine,
    keyboard: Keyboard,
    rules: ListChecks,
    list: ListBullets,
    library: Books,
    inbox: Tray,
    plus: Plus,
    close: X,
    check: Check,
    done: CheckCircle,
    lock: Lock,
    dots: DotsThree,
    trophy: Trophy,
    edit: PencilSimple,
    trash: Trash,
    gear: Gear,
    bell: Bell,
    clock: Clock,
    timer: Timer,
    users: Users,
    copy: Copy,
    merge: ArrowsMerge,
    palette: Palette,
    save: FloppyDisk,
    steam: GameController,
    warning: Warning,
    caret: CaretDown,
    heart: Heart,
    family: UsersThree,
  };

  const Glyph = $derived(icons[name]);
</script>

{#if Glyph}<Glyph {size} {weight} aria-hidden="true" />{/if}
