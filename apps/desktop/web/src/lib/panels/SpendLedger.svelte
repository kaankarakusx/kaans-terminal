<!--
  spend.ledger: harcama (api-eq). Sekmeler: today, 7d, 30d, 12w, 12m; proje ve modele göre.
  Geçmiş günler uygulamanın kendi defterinden gelir (transcript'ler silinse de kalır).
-->
<script lang="ts">
  import Panel from '../Panel.svelte'
  import Sparkline from '../Sparkline.svelte'
  import { engine } from '../engine.svelte'
  import { compact, shortModel, usd } from '../format'
  import { layout } from '../layout.svelte'
  import type { LedgerKey } from '../types'

  const TABS: LedgerKey[] = ['today', '7d', '30d', '12w', '12m']
  let tab = $state<LedgerKey>('today')

  const ledger = $derived(engine.snapshot?.ledger)
  const range = $derived(ledger?.ranges.find((r) => r.key === tab))
  const pending = $derived(tab !== 'today' && !ledger?.ready)
  const cleared = $derived(layout.clearedAt('spend.ledger') > 0)
  // Sparkline tam sayı ister; sentlere çevir.
  const daily = $derived((ledger?.daily ?? []).map((c) => Math.round(c * 100)))
</script>

<Panel tag="spend.ledger" id="spend.ledger" meta={cleared ? '' : range ? `${usd(range.cost)} · ${compact(range.tokens)} tok` : ''}>
  <div class="tabs" role="tablist">
    {#each TABS as t (t)}
      <button role="tab" aria-selected={tab === t} class:on={tab === t} onclick={() => (tab = t)}>{t}</button>
    {/each}
  </div>
  {#if cleared}
    <div class="empty">cleared · right-click to show</div>
  {:else if pending}
    <div class="empty">indexing history…</div>
  {:else if range && range.rows.length}
    <div class="spark" title="last 30 days, api-eq per day"><Sparkline values={daily} label="api-eq per day, last 30 days" /></div>
    {#each range.rows as r (r.project + r.model)}
      <div class="row">
        <span class="pm"><b>{r.project}</b> <span class="m">{shortModel(r.model)}</span></span>
        <span class="tok">{compact(r.tokens)}</span>
        <span class="usd">{usd(r.cost)}</span>
      </div>
    {/each}
  {:else}
    <div class="empty">no spend in this range</div>
  {/if}
</Panel>

<style>
  .tabs {
    position: sticky;
    top: 0;
    z-index: 1;
    display: flex;
    gap: 2px;
    padding: 3px 8px;
    background: var(--panel);
    border-bottom: 1px solid var(--line-soft);
  }
  .tabs button {
    all: unset;
    padding: 0 6px;
    color: var(--dim);
    cursor: pointer;
  }
  .tabs button:hover { color: var(--txt); }
  .tabs button:focus-visible { outline: 1px solid var(--accent); }
  .tabs button.on { color: var(--ink); background: var(--accent); }
  .spark { padding: 4px 8px 2px; --hue: var(--amber); }
  .row {
    display: grid;
    grid-template-columns: 1fr auto auto;
    gap: 10px;
    padding: 1px 8px;
    white-space: nowrap;
  }
  .row:hover { background: rgba(var(--accent-rgb), 0.06); }
  .pm { overflow: hidden; text-overflow: ellipsis; }
  .pm b { color: var(--txt); font-weight: 600; }
  .m { color: var(--dim); font-size: 0.9em; }
  .tok { color: var(--txt3); }
  .usd { color: var(--amber); min-width: 7ch; text-align: right; }
  .empty { color: var(--dim); padding: 12px; }
</style>
