<!--
  Uygulamanın açık terminali (sohbet kartıyla aynı dil): üç ışık (kırmızı = terminali kapat, onaylı; yeşil = aç),
  proje, durum rozeti (RUNNING / READY), çalışan komut ya da "shell ready", klasör. Karta tıklamak terminali açar.
  Aynı pencerede bölünmüş/sekmeli terminaller tek kart: `panes` o penceredeki bütün terminaller (ilki `t`).
-->
<script lang="ts">
  import { confirm } from './dialog.svelte'
  import { duration, tildePath } from './format'
  import { now } from './now.svelte'
  import { openTerminal } from './terminals'
  import type { TerminalView } from './types'
  import { killTerminal } from '../term/api'

  let { t, panes = [t] }: { t: TerminalView; panes?: TerminalView[] } = $props()

  const busy = $derived(panes.some((p) => p.busy))
  const attached = $derived(panes.some((p) => p.attached))
  const running = $derived(panes.filter((p) => p.busy).map((p) => p.title))
  const started = $derived(Math.min(...panes.map((p) => p.started)))

  function end() {
    const many = panes.length > 1
    confirm.ask({
      title: many ? `End ${panes.length} terminals?` : `End "${t.title}"?`,
      body: many
        ? `All terminals in this window (${t.cwd}) are closed${running.length ? ` and ${running.join(', ')} stopped` : ''}.`
        : `The terminal in ${t.cwd} is closed and ${t.title} is stopped.`,
      action: many ? 'end terminals' : 'end terminal',
      danger: true,
      run: async () => {
        for (const p of panes) await killTerminal(p.id)
      },
    })
  }
</script>

<article class="card" class:busy class:winopen={attached}>
  <div class="row top">
    <span class="lights">
      <button class="light red" title="end terminal" aria-label="end terminal" onclick={end}></button>
      <span class="light amber off" aria-hidden="true"></span>
      <button class="light green" title="open terminal" aria-label="open terminal" onclick={() => openTerminal(t.id, t.title)}></button>
    </span>
    <button class="proj" title="open · {t.cwd}" onclick={() => openTerminal(t.id, t.title)}>{t.project}</button>
    {#if attached}<span class="tag win">window</span>{/if}
    {#if panes.length > 1}<span class="tag" title="terminals in this window">{panes.length} panes</span>{/if}
    <span class="grow"></span>
    <span class="badge">{busy ? 'RUNNING' : 'READY'}</span>
  </div>
  <button class="title" class:idle={!busy} title="open terminal" onclick={() => openTerminal(t.id, t.title)}>{busy ? `▸ ${running.join(' · ')}` : '❯ shell ready'}</button>
  <div class="path" title={t.cwd}>{tildePath(t.cwd)} · terminal open {duration((now.value - started) / 1000)}</div>
</article>

<style>
  .card {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 4px;
    padding: 7px 10px 8px;
    border: 1px solid rgba(var(--cyan-rgb), 0.35);
    background: rgba(var(--term-bg-rgb), 0.55);
  }
  .card.busy { border-color: rgba(var(--accent-rgb), 0.6); }
  .winopen { box-shadow: 0 0 0 1px rgba(var(--accent-rgb), 0.5), 0 0 16px rgba(var(--accent-rgb), 0.25); }
  .row { display: flex; align-items: center; gap: 8px; min-width: 0; white-space: nowrap; }
  .grow { flex: 1; }
  .lights { display: flex; gap: 5px; flex: none; }
  .light { all: unset; width: 10px; height: 10px; border-radius: 50%; cursor: pointer; opacity: 0.8; }
  .light:hover, .light:focus-visible { opacity: 1; }
  .light:focus-visible { outline: 1px solid var(--hi); outline-offset: 2px; }
  .red { background: var(--red); }
  .amber { background: var(--amber); }
  .amber.off { opacity: 0.25; cursor: default; }
  .green { background: var(--green); }
  .proj { all: unset; cursor: pointer; color: var(--hi); font-weight: 700; overflow: hidden; text-overflow: ellipsis; min-width: 0; }
  .proj:hover { color: var(--accent); }
  .tag { flex: none; padding: 0 5px; border: 1px solid var(--line); color: var(--txt3); font-size: 0.85em; }
  .tag.win { color: var(--accent); border-color: rgba(var(--accent-rgb), 0.6); }
  .badge {
    flex: none;
    padding: 0 6px;
    border: 1px solid rgba(var(--cyan-rgb), 0.55);
    color: var(--cyan);
    font-weight: 700;
    font-size: 0.88em;
    letter-spacing: 0.05em;
  }
  .busy .badge { border-color: var(--accent); color: var(--accent); box-shadow: 0 0 8px rgba(var(--accent-rgb), 0.4); }
  .title.idle { color: var(--txt3); font-weight: 400; }
  .title { all: unset; cursor: pointer; color: var(--hi); font-size: 1.05em; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .title:hover { text-decoration: underline; text-decoration-color: var(--accent); }
  .title:focus-visible { outline: 1px solid var(--accent); }
  .path { color: var(--dim); font-size: 0.9em; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
