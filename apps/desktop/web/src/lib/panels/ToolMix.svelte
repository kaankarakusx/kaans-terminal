<!-- tool.mix: araç kategorilerinin dağılımı (yatay çubuklar) ve altında 3 sütunlu renk açıklaması. -->
<script lang="ts">
  import Panel from '../Panel.svelte'
  import { engine } from '../engine.svelte'
  import { layout } from '../layout.svelte'
  import { CATEGORIES } from '../types'

  const ID = 'tool.mix'

  const counts = $derived.by(() => {
    const mix = engine.snapshot?.kpis.toolMix ?? {}
    const base = layout.baseline(ID)
    return CATEGORIES.map((c) => ({ cat: c, n: Math.max(0, (mix[c] ?? 0) - (base[c] ?? 0)) }))
  })
  const total = $derived(counts.reduce((a, b) => a + b.n, 0))
  const bars = $derived(counts.filter((c) => c.n > 0).toSorted((a, b) => b.n - a.n))
  const max = $derived(Math.max(1, ...bars.map((b) => b.n)))
</script>

<Panel
  tag="tool.mix"
  id={ID}
  meta="{total} calls"
  onclear={() => ({ ...(engine.snapshot?.kpis.toolMix ?? {}) }) as Record<string, number>}
>
  <div class="wrap">
    <div class="bars">
      {#each bars as b (b.cat)}
        <div class="bar" title="{b.cat}: {b.n} ({Math.round((100 * b.n) / Math.max(1, total))}%)">
          <span class="l">{b.cat}</span>
          <span class="track"><i style:width="{(100 * b.n) / max}%" style:background="var(--cat-{b.cat})"></i></span>
          <span class="n">{b.n}</span>
        </div>
      {:else}
        <div class="empty">no tool calls yet</div>
      {/each}
    </div>
    <div class="legend">
      {#each CATEGORIES as c (c)}
        <span><i style:background="var(--cat-{c})"></i>{c}</span>
      {/each}
    </div>
  </div>
</Panel>

<style>
  .wrap { display: flex; flex-direction: column; justify-content: space-between; min-height: 100%; padding: 4px 8px; gap: 6px; }
  .bars { display: grid; gap: 3px; }
  .bar { display: grid; grid-template-columns: 52px 1fr 3.5ch; align-items: center; gap: 6px; }
  .l { color: var(--txt3); }
  .track { height: 7px; background: var(--line-soft); }
  .track i { display: block; height: 100%; opacity: 0.9; }
  .n { color: var(--txt2); text-align: right; }
  .legend {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 1px 8px;
    color: var(--dim);
    font-size: 0.85em;
  }
  .legend span { display: flex; align-items: center; gap: 4px; }
  .legend i { width: 7px; height: 7px; flex: none; }
  .empty { color: var(--dim); padding: 8px 0; }
</style>
