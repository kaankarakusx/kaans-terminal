<!-- Seni bekleyen oturumlar: ne bekliyor ve ne kadardır. -->
<script lang="ts">
  import Panel from './Panel.svelte'
  import StatusDot from './StatusDot.svelte'
  import { engine } from './engine.svelte'
  import { now } from './now.svelte'
  import { ago } from './format'
  import { focus } from './focus.svelte'
  import { layout } from './layout.svelte'

  // Temizlenince o ana kadar bekleyenler gizlenir; yeniden çalışıp bekleyen geri gelir.
  const waiting = $derived(
    (engine.snapshot?.sessions ?? []).filter(
      (s) => (s.status === 'waiting' || s.status === 'idle') && s.lastActivity > layout.clearedAt('your.call'),
    ),
  )
  const needs = $derived(waiting.filter((s) => s.status === 'waiting').length)
</script>

<Panel tag="your.call" id="your.call" meta="{needs} need you · {waiting.length - needs} your turn" hot={needs > 0}>
  {#each waiting as s (s.id)}
    <button class="row" onclick={() => focus.session(s.id)} title="show in sessions">
      <StatusDot status={s.status} />
      <div class="what">
        <span class="title" style:color="var(--hue{s.hue})">{s.title}</span>
        <span class="why">{s.status === 'waiting' ? (s.waitingFor ?? 'needs approval') : 'your turn'}</span>
      </div>
      <span class="age">{s.lastActivity ? ago(now.value - s.lastActivity) : ''}</span>
    </button>
  {:else}
    <div class="empty">nothing is waiting on you</div>
  {/each}
</Panel>

<style>
  .row {
    all: unset;
    box-sizing: border-box;
    width: 100%;
    cursor: pointer;
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: center;
    gap: 8px;
    padding: 4px 8px;
    border-bottom: 1px dashed var(--line-soft);
  }
  .row:hover { background: rgba(var(--accent-rgb), 0.06); }
  .row:focus-visible { outline: 1px solid var(--accent); outline-offset: -1px; }
  .what { display: grid; min-width: 0; }
  .what > span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .why { color: var(--dim); font-size: 0.9em; }
  .age { color: var(--txt3); }
  .empty { color: var(--dim); padding: 12px; }
</style>
