<!-- research.feed: web aramaları ve getirilen sayfalar (sorgu/URL, hangi oturum). -->
<script lang="ts">
  import Panel from '../Panel.svelte'
  import { engine } from '../engine.svelte'
  import { ago } from '../format'
  import { layout } from '../layout.svelte'
  import { now } from '../now.svelte'

  const ICON = { search: '⌕', fetch: '↗', browser: '◎' } as const
  const items = $derived((engine.snapshot?.web ?? []).filter((w) => w.ts > layout.clearedAt('research.feed')))
</script>

<Panel tag="research.feed" id="research.feed" meta="{items.length} today">
  {#each items as w, i (i + ':' + w.ts)}
    {@const s = engine.sessionsById.get(w.session)}
    <div class="row" title={w.target}>
      <span class="k {w.kind}" title={w.kind}>{ICON[w.kind]}</span>
      <span class="t">{w.target}</span>
      <span class="s" style:color="var(--hue{s?.hue ?? 9})">{w.subagent ? '⬡ ' : ''}{s?.title ?? w.session.slice(0, 8)}</span>
      <span class="a">{ago(now.value - w.ts)}</span>
    </div>
  {:else}
    <div class="empty">no web activity yet</div>
  {/each}
</Panel>

<style>
  .row {
    display: grid;
    grid-template-columns: 14px 1fr minmax(0, 30%) 28px;
    gap: 6px;
    padding: 2px 8px;
    white-space: nowrap;
  }
  .row > span { overflow: hidden; text-overflow: ellipsis; }
  .row:hover { background: rgba(var(--accent-rgb), 0.06); }
  .k { color: var(--cat-web); text-align: center; }
  .k.browser { color: var(--cat-browser); }
  .t { color: var(--txt2); }
  .s { font-size: 0.9em; }
  .a { color: var(--faint); text-align: right; }
  .empty { color: var(--dim); padding: 12px; }
</style>
