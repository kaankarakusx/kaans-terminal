<!--
  Sol: sohbet listesi (OZELLIKLER.md §2.2). Her klasör bir grup; şeridine tıklanınca katlanır. Grupta o klasörde şu an
  açık olanlar kutu kutu: canlı sohbetler (bitmişler gizli) ve uygulamanın açık terminalleri (komut çalışıyor ya da
  hazır). Karta tıklamak sohbeti / terminali açar.
-->
<script lang="ts">
  import Panel from './Panel.svelte'
  import SessionCard from './SessionCard.svelte'
  import TermCard from './TermCard.svelte'
  import { engine } from './engine.svelte'
  import { openChat } from './terminals'
  import type { SessionView, TerminalView } from './types'
  import { windowFor } from '../term/model.svelte'

  const ORDER = { waiting: 0, idle: 1, busy: 2, ended: 3 } as const

  const groups = $derived.by(() => {
    const map = new Map<string, { name: string; sessions: SessionView[]; terminals: TerminalView[][] }>()
    // Aynı penceredeki (bölme/sekme) terminaller tek kart. Pencere düzeni localStorage'da; snapshot her saniye
    // geldiği için bu türetme de her saniye yeniden okunur.
    const byWin = new Map<string, TerminalView[]>()
    const group = (cwd: string, name: string) => {
      let g = map.get(cwd)
      if (!g) map.set(cwd, (g = { name, sessions: [], terminals: [] }))
      return g
    }
    // Yalnızca şu an açık olanlar: bitmiş sohbetler listede yok
    for (const s of engine.snapshot?.sessions ?? []) if (s.status !== 'ended') group(s.cwd || s.project, s.project).sessions.push(s)
    // İçinde Claude çalışan terminal sohbet kartıyla görünür; içinde uygulama (npm run dev…) çalışan terminal yalnızca
    // orbit.map'te araba olarak (tıklayınca açılır); diğer her açık terminal kendi kutusuyla
    const withApp = new Set((engine.snapshot?.apps ?? []).map((a) => a.terminal).filter(Boolean))
    for (const t of engine.snapshot?.terminals ?? []) {
      if (t.chat || withApp.has(t.id)) continue
      const win = windowFor(t.id)
      const same = win ? byWin.get(win) : undefined
      if (same) same.push(t)
      else {
        const card = [t]
        if (win) byWin.set(win, card)
        group(t.cwd, t.project).terminals.push(card)
      }
    }
    const rank = (g: { sessions: SessionView[]; terminals: TerminalView[][] }) =>
      g.sessions.length ? ORDER[g.sessions[0].status] : g.terminals.some((c) => c.some((t) => t.busy)) ? 2 : 3
    return [...map.entries()]
      .map(([cwd, g]) => ({ cwd, ...g }))
      .sort((a, b) => rank(a) - rank(b))
  })

  /** katlanan klasörler (cwd) ve tek satıra indirilen kartlar (oturum id) */
  let collapsed = $state<Record<string, boolean>>({})
  let folded = $state<Record<string, boolean>>({})
  const live = $derived(engine.snapshot?.kpis.sessionsLive ?? 0)
  const terms = $derived(groups.reduce((n, g) => n + g.terminals.length, 0))
  const openTerms = $derived(new Set((engine.snapshot?.terminals ?? []).filter((t) => t.attached).map((t) => t.id)))
</script>

<Panel tag="sessions" id="sessions" meta="{live} live · {engine.snapshot?.sessions.length ?? 0} today{terms ? ` · ${terms} term` : ''}">
  <div class="list">
    {#each groups as g (g.cwd)}
      <div class="folder">
        <button class="strip" title={g.cwd} onclick={() => (collapsed[g.cwd] = !collapsed[g.cwd])}>
          <span class="caret">{collapsed[g.cwd] ? '▸' : '▾'}</span>
          <span class="name">{g.name}</span>
          <span class="count">{g.sessions.length + g.terminals.length}</span>
        </button>
        {#if !collapsed[g.cwd]}
          <div class="cards">
            {#each g.sessions as s (s.id)}
              <SessionCard
                {s}
                folded={folded[s.id]}
                winopen={!!s.terminal && openTerms.has(s.terminal)}
                onfold={() => (folded[s.id] = !folded[s.id])}
                onopen={() => openChat(s)}
              />
            {/each}
            {#each g.terminals as c (c[0].id)}
              <TermCard t={c[0]} panes={c} />
            {/each}
          </div>
        {/if}
      </div>
    {:else}
      <div class="empty">nothing running right now</div>
    {/each}
  </div>
</Panel>

<style>
  .list { display: grid; grid-template-columns: minmax(0, 1fr); gap: 8px; padding: 8px; }
  .folder { border: 1px solid var(--line-soft); }
  .strip {
    all: unset;
    box-sizing: border-box;
    width: 100%;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 3px 8px;
    cursor: pointer;
    color: var(--txt2);
    background: rgba(var(--accent-rgb), 0.06);
  }
  .strip:hover { color: var(--hi); background: rgba(var(--accent-rgb), 0.12); }
  .strip:focus-visible { outline: 1px solid var(--accent); }
  .caret { color: var(--accent); width: 1em; }
  .name { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: 600; }
  .count { color: var(--dim); }
  .cards { display: grid; grid-template-columns: minmax(0, 1fr); gap: 4px; padding: 4px; }
  .empty { color: var(--dim); padding: 12px; }
</style>
