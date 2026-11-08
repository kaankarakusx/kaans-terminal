<!-- workers: çalışan alt ajanlar — rol, açıklama, o anki araç, süre. -->
<script lang="ts">
  import Panel from '../Panel.svelte'
  import { engine } from '../engine.svelte'
  import { ago } from '../format'
  import { layout } from '../layout.svelte'
  import { now } from '../now.svelte'

  const workers = $derived(
    (engine.snapshot?.sessions ?? [])
      .flatMap((s) => s.subagents.map((a) => ({ ...a, session: s })))
      .filter((a) => a.running && a.started > layout.clearedAt('workers'))
      .toSorted((a, b) => a.started - b.started),
  )
</script>

<Panel tag="workers" id="workers" meta="{workers.length} running">
  {#each workers as w (w.id)}
    <div class="row" title="{w.session.title} · {w.description}">
      <span class="hex" style:color="var(--hue{w.session.hue})">⬡</span>
      <span class="role">{w.kind}</span>
      <span class="desc">{w.description || w.session.title}</span>
      <span class="tool">{w.currentTool ?? w.lastTool ?? '—'}</span>
      <span class="dur">{w.started ? ago(now.value - w.started) : ''}</span>
    </div>
  {:else}
    <div class="empty">no subagents running</div>
  {/each}
</Panel>

<style>
  .row {
    display: grid;
    grid-template-columns: 12px minmax(0, auto) 1fr auto 30px;
    gap: 6px;
    padding: 2px 8px;
    white-space: nowrap;
    align-items: center;
  }
  .row > span { overflow: hidden; text-overflow: ellipsis; }
  .row:hover { background: rgba(var(--violet-rgb), 0.08); }
  .role { color: var(--violet); max-width: 14ch; }
  .desc { color: var(--txt2); }
  .tool { color: var(--txt3); max-width: 12ch; }
  .dur { color: var(--faint); text-align: right; }
  .empty { color: var(--dim); padding: 12px; }
</style>
