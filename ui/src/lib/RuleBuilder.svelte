<script lang="ts">
  // Builds a condition with AND, OR and NOT: a group of tests and nested groups. Edits the
  // condition in place, so it works on reactive state directly.
  import RuleBuilder from './RuleBuilder.svelte';
  import { FIELDS, OPS, fieldById, groupItems } from './criteria';
  import type { Cond, Test } from './criteria';
  import { app } from './store.svelte';
  import Icon from './Icon.svelte';

  let { cond, depth = 0, onremove }: { cond: Cond; depth?: number; onremove?: () => void } = $props();

  type Mode = 'all' | 'any' | 'none';
  const mode = $derived<Mode>('all' in cond ? 'all' : 'any' in cond ? 'any' : 'none');
  const items = $derived('not' in cond ? groupItems(cond.not) : groupItems(cond));
  const groups = $derived([...new Set(FIELDS.map((f) => f.group))]);

  function setMode(m: Mode) {
    const list = [...items];
    const c = cond as Record<string, unknown>;
    for (const k of Object.keys(c)) delete c[k];
    if (m === 'all') c.all = list;
    else if (m === 'any') c.any = list;
    else c.not = { any: list };
  }
  const list = () => ('not' in cond ? groupItems(cond.not) : groupItems(cond));
  const add = (c: Cond) => list().push(c);
  const removeAt = (i: number) => list().splice(i, 1);

  function setField(t: Test, id: string) {
    const f = fieldById(id)!;
    t.field = f.id;
    t.op = OPS[f.type][0][0];
    t.value = f.type === 'bool' ? true : f.type === 'set' ? [] : f.type === 'choice' ? f.options?.[0][0] : '';
  }
  function setOp(t: Test, op: string) {
    t.op = op;
    if (op === 'between') t.value = [null, null];
    else if (op === 'in') t.value = Array.isArray(t.value) ? t.value : t.value ? [t.value] : [];
    else if (fieldById(t.field)?.type === 'choice' && Array.isArray(t.value)) t.value = t.value[0] ?? fieldById(t.field)?.options?.[0][0];
    else if (Array.isArray(t.value) && fieldById(t.field)?.type !== 'set') t.value = '';
  }
  const numberOrNull = (v: string) => (v.trim() === '' ? null : Number(v));

  /** Options for set and multi-choice values. */
  function options(t: Test): [string | number, string][] {
    const f = fieldById(t.field);
    if (f?.options) return f.options;
    if (t.field === 'tags') return app.tagChoices.map((x) => [x.id, x.name]);
    if (t.field === 'features') return (app.library?.flags ?? []).map((x) => [x.id, x.name.charAt(0).toUpperCase() + x.name.slice(1)]);
    if (t.field === 'collections') return app.collections.filter((c) => !c.dynamic).map((c) => [c.id, c.name]);
    return [];
  }
  const picked = (t: Test) => (Array.isArray(t.value) ? t.value : []);
  const toggle = (t: Test, v: string | number) => {
    const cur = picked(t);
    t.value = cur.some((x) => x === v) ? cur.filter((x) => x !== v) : [...cur, v];
  };

  const ctl = 'select select-xs h-7 bg-base-100';
</script>

<div class="flex flex-col gap-1.5 {depth ? 'rounded-box border border-base-300 p-2.5' : ''}">
  <div class="flex items-center gap-2 text-[13px]">
    <span>Match</span>
    <select class={ctl} value={mode} onchange={(e) => setMode(e.currentTarget.value as Mode)} aria-label="How to combine">
      <option value="all">all of these</option>
      <option value="any">any of these</option>
      <option value="none">none of these</option>
    </select>
    <span class="flex-1"></span>
    {#if onremove}<button class="btn btn-ghost btn-xs btn-square" aria-label="Remove group" onclick={onremove}><Icon name="close" size={13} /></button>{/if}
  </div>

  {#each items as item, i (i)}
    {#if 'field' in item}
      {@const f = fieldById(item.field)}
      <div class="flex flex-wrap items-center gap-1.5">
        <select class="{ctl} w-40" value={item.field} onchange={(e) => setField(item, e.currentTarget.value)} aria-label="Filter">
          {#each groups as g}<optgroup label={g}>{#each FIELDS.filter((x) => x.group === g) as x}<option value={x.id}>{x.label}</option>{/each}</optgroup>{/each}
        </select>
        {#if f && f.type !== 'bool'}
          <select class="{ctl} w-28" value={item.op} onchange={(e) => setOp(item, e.currentTarget.value)} aria-label="Comparison">
            {#each OPS[f.type] as [op, label]}<option value={op}>{label}</option>{/each}
          </select>
        {/if}
        {#if f?.type === 'text'}
          <input class="input input-xs h-7 min-w-24 flex-1" value={String(item.value ?? '')} oninput={(e) => (item.value = e.currentTarget.value)} aria-label="Text" placeholder="Type a name" />
        {:else if f?.type === 'num' && item.op === 'between'}
          {@const r = Array.isArray(item.value) ? item.value : [null, null]}
          <input type="number" step="any" class="input input-xs h-7 w-20" value={r[0] ?? ''} oninput={(e) => (item.value = [numberOrNull(e.currentTarget.value), r[1]])} aria-label="From" />
          <span class="text-xs text-muted">and</span>
          <input type="number" step="any" class="input input-xs h-7 w-20" value={r[1] ?? ''} oninput={(e) => (item.value = [r[0], numberOrNull(e.currentTarget.value)])} aria-label="To" />
          {#if f.unit}<span class="text-xs text-muted">{f.id === 'last_played_days' ? 'days ago' : f.unit}</span>{/if}
          {#if r[0] != null && r[1] != null && Number(r[0]) > Number(r[1])}<span class="text-xs text-error">From must be at most To.</span>{/if}
        {:else if f?.type === 'num' && item.op !== 'known' && item.op !== 'unknown'}
          <input type="number" step="any" class="input input-xs h-7 w-24" value={item.value ?? ''} oninput={(e) => (item.value = numberOrNull(e.currentTarget.value))} aria-label="Number" />
          {#if f.unit}<span class="text-xs text-muted">{f.id === 'last_played_days' ? 'days ago' : f.unit}</span>{/if}
        {:else if f?.type === 'bool'}
          <select class="{ctl} w-20" value={item.value === false ? 'no' : 'yes'} onchange={(e) => (item.value = e.currentTarget.value === 'yes')} aria-label="Yes or no">
            <option value="yes">yes</option><option value="no">no</option>
          </select>
        {:else if f?.type === 'choice' && item.op !== 'in'}
          <select class="{ctl} w-36" value={String(item.value ?? '')} onchange={(e) => (item.value = e.currentTarget.value)} aria-label="Value">
            {#each f.options ?? [] as [v, label]}<option value={v}>{label}</option>{/each}
          </select>
        {:else if f && f.type !== 'num'}
          <select class="{ctl} w-44" value="" onchange={(e) => { const raw = e.currentTarget.value; const o = options(item).find(([v]) => String(v) === raw); if (o) toggle(item, o[0]); e.currentTarget.value = ''; }} aria-label="Add a value">
            <option value="" disabled>{picked(item).length ? 'Add another…' : 'Choose…'}</option>
            {#each options(item) as [v, label]}<option value={String(v)}>{label}</option>{/each}
          </select>
          {#each picked(item) as v}
            <button class="inline-flex h-6 items-center gap-1 rounded-field border border-base-300 px-1.5 text-xs" onclick={() => toggle(item, v as string | number)}>
              {options(item).find(([o]) => o === v)?.[1] ?? v} <Icon name="close" size={10} />
            </button>
          {/each}
        {/if}
        <button class="btn btn-ghost btn-xs btn-square ml-auto" aria-label="Remove" onclick={() => removeAt(i)}><Icon name="close" size={13} /></button>
      </div>
    {:else}
      <RuleBuilder cond={item} depth={depth + 1} onremove={() => removeAt(i)} />
    {/if}
  {/each}

  <div class="flex gap-1">
    <button class="btn btn-ghost btn-xs" onclick={() => add({ field: 'publisher', op: 'contains', value: '' })}><Icon name="plus" size={12} /> Add condition</button>
    {#if depth < 2}<button class="btn btn-ghost btn-xs" onclick={() => add({ any: [] })}><Icon name="plus" size={12} /> Add group</button>{/if}
  </div>
</div>
