<!--
  Üst bar limitleri (OZELLIKLER.md §2.1): araç başına 5 saatlik ve haftalık kullanım, çubukla ve sıfırlanma saatiyle.
  Veri yoksa gizli. Claude: statusLine entegrasyonu; Codex: son rollout (yaklaşık).
-->
<script lang="ts">
  import { engine } from './engine.svelte'
  import { now } from './now.svelte'
  import type { LimitWindow } from './types'

  const HOT = 80
  const limits = $derived((engine.snapshot?.limits ?? []).filter((l) => l.fiveHour || l.weekly))

  function resets(w: LimitWindow): string {
    if (!w.resetsAt) return ''
    const ms = w.resetsAt * 1000 - now.value
    if (ms <= 0) return 'resetting'
    const m = Math.round(ms / 60000)
    if (m < 60) return `resets in ${m}m`
    const d = new Date(w.resetsAt * 1000)
    return m < 24 * 60
      ? `resets ${d.toLocaleTimeString('en-GB', { hour: '2-digit', minute: '2-digit' })}`
      : `resets ${d.toLocaleDateString('en-GB', { weekday: 'short' })} ${d.toLocaleTimeString('en-GB', { hour: '2-digit', minute: '2-digit' })}`
  }
</script>

{#each limits as l (l.tool)}
  <div class="limits" title="{l.tool} usage limits{l.tool === 'codex' ? ' (from the last Codex session, approximate)' : ''}">
    <span class="tool">{l.tool}</span>
    {#each [['5h', l.fiveHour], ['week', l.weekly]] as const as [name, w] (name)}
      {#if w}
        <div class="w" class:hot={w.usedPct >= HOT} title="{name}: {Math.round(w.usedPct)}% · {resets(w)}">
          <span class="n">{name} <b>{Math.round(w.usedPct)}%</b></span>
          <span class="bar"><i style:width="{Math.min(100, w.usedPct)}%"></i></span>
        </div>
      {/if}
    {/each}
  </div>
{/each}

<style>
  .limits { display: flex; align-items: center; gap: 8px; flex: none; }
  .tool { color: var(--dim); font-size: 0.85em; }
  .w { display: grid; gap: 2px; min-width: 64px; }
  .n { color: var(--dim); font-size: 0.85em; white-space: nowrap; }
  .n b { color: var(--hi); }
  .bar { height: 3px; background: var(--line-soft); }
  .bar i { display: block; height: 100%; background: var(--accent); }
  .hot .n b { color: var(--red); }
  .hot .bar i { background: var(--red); box-shadow: 0 0 8px rgba(var(--red-rgb), 0.8); }
</style>
