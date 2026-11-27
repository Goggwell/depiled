<script lang="ts">
  // Every icon in the app comes from Phosphor, kept in icons.ts in just the weights used.
  // Names here are what the screens ask for, so a different icon can be swapped in one place
  // (scripts/icons.mjs).
  import { ICONS } from './icons';
  import type { Weight } from './icons';

  let { name, size = 18, weight = 'regular' }: { name: string; size?: number; weight?: Weight } = $props();

  const shape = $derived(ICONS[name]?.[weight] ?? ICONS[name]?.regular);
</script>

{#if shape}
  <svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="currentColor" aria-hidden="true">{@html shape}</svg>
{/if}
