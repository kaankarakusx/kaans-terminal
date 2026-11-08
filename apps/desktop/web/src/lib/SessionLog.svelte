<!-- Canlı araç akışı: zaman | oturum | araç | hedef | durum. Yapay zekânın cevapları "reply" satırı. -->
<script lang="ts">
  import Panel from './Panel.svelte'
  import { engine } from './engine.svelte'
  import { clock } from './format'
  import { layout } from './layout.svelte'

  const rows = $derived(
    engine.events
      .filter((e) => e.kind !== 'toolError' && e.ts > layout.clearedAt('session.log'))
      .toReversed(),
  )
</script>

<Panel tag="session.log" id="session.log" meta="{rows.length} events">
  <div class="log">
    {#each rows as e, i (i + ':' + e.ts + ':' + (e.id ?? e.kind))}
      {@const s = engine.sessionsById.get(e.session)}
      {@const failed = e.id != null && engine.failed.has(e.id)}
      <div class="row" class:reply={e.kind === 'reply'} class:prompt={e.kind === 'prompt'}>
        <span class="t">{clock(e.ts)}</span>
        <span class="s" style:color="var(--hue{s?.hue ?? 9})" title={s?.title}>{s?.title ?? e.session.slice(0, 8)}</span>
        <span class="k" style:color={e.category ? `var(--cat-${e.category})` : undefined}>
          {e.kind === 'tool' ? (e.subagent ? '⬡' : '') + e.tool : e.kind}
        </span>
        <span class="g" title={e.target}>{e.target ?? ''}</span>
        <span class="r" class:err={failed}>{e.kind === 'tool' ? (failed ? 'err' : 'ok') : ''}</span>
      </div>
    {:else}
      <div class="empty">waiting for activity…</div>
    {/each}
  </div>
</Panel>

<style>
  .row {
    display: grid;
    grid-template-columns: 58px 82px 50px 1fr auto;
    gap: 8px;
    padding: 1px 8px;
    white-space: nowrap;
  }
  .row > span { overflow: hidden; text-overflow: ellipsis; }
  .row:hover { background: rgba(var(--accent-rgb), 0.06); }
  .t { color: var(--faint); }
  .k { color: var(--txt3); }
  .g { color: var(--txt2); }
  .r { color: var(--faint); }
  .r.err { color: var(--red); font-weight: 700; }
  .reply .g { color: var(--txt3); font-style: italic; }
  .prompt .g { color: var(--hi); }
  .empty { color: var(--dim); padding: 12px; }
</style>
