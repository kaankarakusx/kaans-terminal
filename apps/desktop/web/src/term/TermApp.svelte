<!--
  Sohbet / terminal penceresi (TASARIM.md §4.2, OZELLIKLER.md §3): başlık satırı, sekmeler, bölmeler.
  Pencere ya da sekme kapanınca oturumlar çalışmaya devam eder; End Session onları bitirir.
-->
<script lang="ts">
  import { onMount } from 'svelte'
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
  import ContextMenu from '../lib/ContextMenu.svelte'
  import { engine, inTauri } from '../lib/engine.svelte'
  import { compact, duration, prettyModel, usd } from '../lib/format'
  import { now } from '../lib/now.svelte'
  import { STATUS_LABEL, type SessionView } from '../lib/types'
  import { menu } from '../lib/menu.svelte'
  import { isMac, label as keyLabel, shortcuts } from '../lib/shortcuts.svelte'
  import PaneTree from './PaneTree.svelte'
  import TabBar from './TabBar.svelte'
  import { panes } from './TerminalPane.svelte'
  import * as api from './api'
  import { WindowModel, leaves, type Side } from './model.svelte'

  const params = new URLSearchParams(location.hash.slice(1))
  const winLabel = params.get('term') ?? 'term-demo'
  const model = new WindowModel(winLabel)

  let hostSessions = $state<api.SessionInfo[]>([])
  let ready = $state(false)
  let note = $state<string | null>(null)
  let noteTimer: ReturnType<typeof setTimeout> | undefined
  let finding = $state(false)
  let query = $state('')
  let findInput = $state<HTMLInputElement>()
  let pinned = $state(false)
  let confirmEnd = $state<{ running: string[] } | null>(null)
  let dropHint = $state<{ pane: string; side: Side } | null>(null)

  const byId = $derived(new Map(hostSessions.map((s) => [s.id, s])))
  const active = $derived(model.activePane)
  const activeSession = $derived(active ? byId.get(active.session) : undefined)
  const chat = $derived(active ? engine.snapshot?.sessions.find((s) => s.terminal === active.session) : undefined)

  // Başlık rozeti: durum ve süresi (çalışıyorsa turun, bekliyorsa beklemenin ne kadardır sürdüğü).
  type State = 'busy' | 'waiting' | 'idle' | 'ended' | 'running' | 'shell'
  const winState: State = $derived(
    chat ? chat.status : activeSession?.busy ? 'running' : 'shell',
  )
  const BADGE: Record<State, string> = {
    busy: 'WORKING',
    waiting: 'NEEDS YOU',
    idle: 'YOUR TURN',
    ended: 'ENDED',
    running: 'RUNNING',
    shell: 'TERMINAL',
  }
  const since = $derived(
    chat?.status === 'busy' ? chat.turnStarted : chat?.status === 'waiting' ? chat.lastActivity : 0,
  )
  const elapsed = $derived.by(() => {
    if (!since) return ''
    const s = Math.max(0, Math.floor((now.value - since) / 1000))
    const h = Math.floor(s / 3600)
    const mm = String(Math.floor((s % 3600) / 60)).padStart(2, '0')
    const ss = String(s % 60).padStart(2, '0')
    return h ? `${h}:${mm}:${ss}` : `${mm}:${ss}`
  })
  const project = $derived((chat?.cwd || activeSession?.cwd || '').split('/').filter(Boolean).at(-1) ?? '')
  // Git dalı (düz terminallerde bilgi şeridinde): 5 sn'de bir, yalnızca öndeki bölmenin klasörü için.
  let branch = $state<string | null>(null)
  $effect(() => {
    const cwd = activeSession?.cwd
    if (!cwd) return
    let alive = true
    const load = () => api.gitBranch(cwd).then((b) => alive && (branch = b)).catch(() => {})
    void load()
    const t = setInterval(load, 5000)
    return () => {
      alive = false
      clearInterval(t)
    }
  })
  const shellName = $derived(activeSession?.command[0]?.split('/').at(-1) ?? 'shell')
  const tilde = (p: string) => p.replace(/^\/Users\/[^/]+/, '~').replace(/^\/home\/[^/]+/, '~')

  // Proje adı zaten solda: düz terminalde başlık çalışan komut ya da kabuğun adı.
  const headTitle = $derived(
    chat ? chat.title : activeSession?.busy && activeSession.fgName ? activeSession.fgName : shellName,
  )

  const CAT_LABEL: Record<string, string> = {
    bash: 'shell', read: 'read', edit: 'edit', search: 'find', web: 'web', browser: 'browse',
    agent: 'agent', mcp: 'mcp', plan: 'plan', other: 'tool',
  }
  /** Çip metni: yol ise yalnızca dosya adı, değilse (komut, sorgu) olduğu gibi. */
  const chipText = (t: string) => (/^[~./]?[^\s]*\/[^\s]+$/.test(t) ? (t.split('/').filter(Boolean).at(-1) ?? t) : t)

  const norm = (p: string) => p.replace(/\/+$/, '')
  /** Bilgi şeridinin kaynağı: bu terminaldeki sohbet, yoksa aynı klasörde en son etkin sohbet. */
  const info = $derived.by(() => {
    if (chat) return chat
    const cwd = activeSession?.cwd
    if (!cwd) return undefined
    return engine.snapshot?.sessions
      .filter((x) => norm(x.cwd) === norm(cwd))
      .reduce<SessionView | undefined>((best, x) => (!best || x.lastActivity > best.lastActivity ? x : best), undefined)
  })
  const SPARK = 20
  const spark = $derived.by(() => {
    const out = new Array<number>(SPARK).fill(0)
    if (!info) return out
    const end = now.value
    for (const e of engine.events) {
      if (e.session !== info.id || e.kind !== 'tool') continue
      const k = SPARK - 1 - Math.floor((end - e.ts) / 30_000)
      if (k >= 0 && k < SPARK) out[k]++
    }
    const max = Math.max(1, ...out)
    return out.map((v) => (v ? 15 + (85 * v) / max : 6))
  })

  const recentTools = $derived(
    chat ? engine.events.filter((e) => e.session === chat.id && e.kind === 'tool').slice(-24) : [],
  )

  function flash(text: string) {
    note = text
    clearTimeout(noteTimer)
    noteTimer = setTimeout(() => (note = null), 1600)
  }

  function sessionTitle(id: string): { title: string; busy: boolean; chat: boolean } {
    const c = engine.snapshot?.sessions.find((s) => s.terminal === id)
    if (c) return { title: c.title, busy: c.status === 'busy', chat: true }
    const s = byId.get(id)
    if (!s) return { title: 'terminal', busy: false, chat: false }
    const shell = s.command[0]?.split('/').at(-1) ?? 'shell'
    const where = s.cwd.split('/').at(-1) || s.cwd
    return { title: s.busy && s.fgName ? `${s.fgName} · ${where}` : `${shell} · ${where}`, busy: s.busy, chat: false }
  }

  const titleOf = (i: number) => {
    const t = model.tabs[i]
    const leaf = t && (leaves(t.root).find((l) => l.id === t.active) ?? leaves(t.root)[0])
    return leaf ? sessionTitle(leaf.session) : { title: '', busy: false, chat: false }
  }

  // Pencere başlığı: öndeki sekme.
  $effect(() => {
    const t = active ? sessionTitle(active.session).title : "Kaan's Terminal"
    if (inTauri) void getCurrentWindow().setTitle(t)
    else document.title = t
  })

  async function closeWindow() {
    model.save()
    if (inTauri) await getCurrentWindow().close()
  }

  async function newSession(cwd: string | null): Promise<string> {
    const pc = active ? panes.get(active.id) : undefined
    return api.createTerminal(cwd, pc?.term.cols ?? 100, pc?.term.rows ?? 30)
  }

  const cwdOf = (session: string) => byId.get(session)?.cwd ?? null

  // ───────────────────────── eylemler ─────────────────────────

  async function run(action: string) {
    const pc = active ? panes.get(active.id) : undefined
    const cwd = active ? cwdOf(active.session) : null
    switch (action) {
      case 'term.newTab':
        model.addTab(await newSession(cwd))
        break
      case 'term.splitRight':
        model.split('row', await newSession(cwd))
        break
      case 'term.splitDown':
        model.split('col', await newSession(cwd))
        break
      case 'term.nextPane':
        model.cyclePane(1)
        break
      case 'term.prevPane':
        model.cyclePane(-1)
        break
      case 'term.closePane':
        if (active && model.closePane(active.id)) await closeWindow()
        break
      case 'term.reopenTab':
        model.reopen((s) => byId.has(s))
        break
      case 'term.moveTabLeft':
        model.moveTab(model.activeTab, model.activeTab - 1)
        break
      case 'term.moveTabRight':
        model.moveTab(model.activeTab, model.activeTab + 1)
        break
      case 'term.growLeft':
        model.resizeActive('row', -0.05)
        break
      case 'term.growRight':
        model.resizeActive('row', 0.05)
        break
      case 'term.growUp':
        model.resizeActive('col', -0.05)
        break
      case 'term.growDown':
        model.resizeActive('col', 0.05)
        break
      case 'term.find':
        finding = true
        queueMicrotask(() => findInput?.select())
        break
      case 'term.findNext':
        if (query) pc?.search.findNext(query, searchOpts)
        break
      case 'term.findPrev':
        if (query) pc?.search.findPrevious(query, searchOpts)
        break
      case 'term.useSelection':
        if (pc?.term.hasSelection()) {
          query = pc.term.getSelection()
          finding = true
        }
        break
      case 'term.clear':
        pc?.term.clear()
        break
      case 'term.copyLast':
        await copyLastAnswer()
        break
      case 'term.fullscreen':
        if (inTauri) {
          const w = getCurrentWindow()
          await w.setFullscreen(!(await w.isFullscreen()))
        }
        break
      case 'term.endSession':
        askEnd()
        break
    }
  }

  const searchOpts = {
    decorations: {
      matchBackground: '#5b3a12',
      activeMatchBackground: '#f88008',
      matchOverviewRuler: '#ffb057',
      activeMatchColorOverviewRuler: '#f88008',
    },
  }

  async function copyLastAnswer() {
    if (!active) return
    const text = await api.lastReply(active.session)
    if (!text) return flash('no answer to copy yet')
    await navigator.clipboard.writeText(text)
    flash(`copied last answer · ${text.split('\n').length} lines`)
  }

  function copySelection(): boolean {
    const pc = active ? panes.get(active.id) : undefined
    if (!pc?.term.hasSelection()) return false
    const text = pc.term.getSelection()
    void navigator.clipboard.writeText(text)
    const n = text.split('\n').length
    flash(`copied ${n} line${n === 1 ? '' : 's'}`)
    return true
  }

  /** Geçmişte gezinme. Claude'un tam ekran görünümünde (alternatif ekran) tuşlar uygulamaya gider. */
  function scrollKey(e: KeyboardEvent): boolean {
    const pc = active ? panes.get(active.id) : undefined
    if (!pc) return false
    const alt = pc.term.buffer.active.type === 'alternate'
    const map: Record<string, [() => void, string]> = {
      ArrowUp: [() => pc.term.scrollLines(-1), '\x1b[A'],
      ArrowDown: [() => pc.term.scrollLines(1), '\x1b[B'],
      PageUp: [() => pc.term.scrollPages(-1), '\x1b[5~'],
      PageDown: [() => pc.term.scrollPages(1), '\x1b[6~'],
      Home: [() => pc.term.scrollToTop(), '\x1b[H'],
      End: [() => pc.term.scrollToBottom(), '\x1b[F'],
    }
    const m = map[e.key]
    if (!m) return false
    if (alt) pc.send(m[1])
    else m[0]()
    return true
  }

  const mod = (e: KeyboardEvent) => (isMac ? e.metaKey : e.ctrlKey)

  /** Bu tuş pencereye mi ait (xterm terminale göndermesin)? */
  function isAppShortcut(e: KeyboardEvent): boolean {
    if (shortcuts.match(e, ['term'])) return true
    if (mod(e) && !e.altKey && /^Digit[1-9]$/.test(e.code)) return true
    if (mod(e) && !e.shiftKey && !e.altKey && ['ArrowUp', 'ArrowDown', 'PageUp', 'PageDown', 'Home', 'End'].includes(e.key)) return true
    if (mod(e) && e.code === 'KeyC' && !e.shiftKey) return true
    return false
  }

  function onkeydown(e: KeyboardEvent) {
    if (confirmEnd) {
      if (e.key === 'Escape') confirmEnd = null
      if (e.key === 'Enter') void endNow()
      return
    }
    const action = shortcuts.match(e, ['term'])
    if (action) {
      e.preventDefault()
      e.stopPropagation()
      void run(action)
      return
    }
    if (mod(e) && !e.altKey && /^Digit[1-9]$/.test(e.code)) {
      e.preventDefault()
      model.selectTab(Number(e.code.slice(5)) - 1)
      return
    }
    if (mod(e) && e.code === 'KeyC' && !e.shiftKey && copySelection()) {
      e.preventDefault()
      return
    }
    if (mod(e) && !e.shiftKey && !e.altKey && scrollKey(e)) {
      e.preventDefault()
    }
  }

  // ───────────────────────── End Session ─────────────────────────

  /** Sadece öndeki sekmeyi bitirir; içeride hâlâ çalışan bir şey varsa önce sorar. */
  function askEnd() {
    const t = model.tab
    if (!t) return
    const running = leaves(t.root)
      .map((l) => byId.get(l.session))
      .filter((s): s is api.SessionInfo => !!s?.busy)
      .map((s) => s.fgName ?? 'a process')
    if (running.length || chat?.status === 'busy') confirmEnd = { running: running.length ? running : ['claude'] }
    else void endNow()
  }

  async function endNow() {
    confirmEnd = null
    const t = model.tab
    if (!t) return
    const sessions = leaves(t.root).map((l) => l.session)
    await Promise.all(sessions.map((s) => api.killTerminal(s)))
    if (model.closeTab()) await closeWindow()
  }

  // ───────────────────────── sağ tık ─────────────────────────

  function paneMenu(e: MouseEvent, paneId: string) {
    e.preventDefault()
    model.focusPane(paneId)
    const pc = panes.get(paneId)
    const k = (id: string) => keyLabel(shortcuts.combo(id))
    menu.show(e.clientX, e.clientY, [
      { label: `copy  ${isMac ? '⌘C' : 'Ctrl+C'}`, disabled: !pc?.term.hasSelection(), action: copySelection },
      {
        label: `paste  ${isMac ? '⌘V' : 'Ctrl+V'}`,
        action: async () => {
          const text = await navigator.clipboard.readText().catch(() => '')
          if (text) pc?.term.paste(text)
        },
      },
      { label: `copy last answer  ${k('term.copyLast')}`, action: copyLastAnswer },
      'sep',
      { label: `find  ${k('term.find')}`, action: () => run('term.find') },
      { label: `clear  ${k('term.clear')}`, action: () => pc?.term.clear() },
      'sep',
      { label: `split right  ${k('term.splitRight')}`, action: () => run('term.splitRight') },
      { label: `split down  ${k('term.splitDown')}`, action: () => run('term.splitDown') },
    ])
  }

  function onDragPane(hint: { pane: string; side: Side } | null, drop: boolean, tab: number) {
    dropHint = drop ? null : hint
    if (drop && hint) model.dropTabOnPane(tab, hint.pane, hint.side)
  }

  // ───────────────────────── yaşam döngüsü ─────────────────────────

  onMount(() => {
    void engine.start()
    const offs: (() => void)[] = []
    const open = (session: string) => {
      if (!session) return
      if (!model.reveal(session)) model.addTab(session)
    }
    void (async () => {
      const st = await api.termState().catch(() => ({ sessions: [], leftovers: [] }))
      hostSessions = st.sessions
      if (inTauri) {
        // Önceki çalıştırmadan kalan sekmelerden ölmüş oturumlar temizlenir.
        const alive = new Set(st.sessions.map((s) => s.id))
        for (const s of model.sessions()) if (!alive.has(s)) model.dropSession(s)
      }
      const want = params.get('open')
      if (want) open(want)
      if (!model.tabs.length) {
        if (inTauri) return closeWindow()
        model.addTab(await api.createTerminal(null))
      }
      ready = true
      offs.push(await api.onSessions((s) => (hostSessions = s.sessions)))
      offs.push(
        await api.onExit((id) => {
          // Kabuk kapandı: bölme kısa bir süre sonra kapanır (son çıktı görülsün).
          setTimeout(() => {
            if (model.has(id) && model.dropSession(id)) void closeWindow()
          }, 600)
        }),
      )
      if (inTauri) {
        offs.push(await getCurrentWebviewWindow().listen<string>('term://open', (e) => open(e.payload)))
        // Finder'dan sürüklenen dosyalar: yolları öndeki bölmeye yapıştır (Claude resim yolunu tanır).
        offs.push(
          await getCurrentWebviewWindow().onDragDropEvent((e) => {
            if (e.payload.type !== 'drop' || !active) return
            const quoted = e.payload.paths.map((p) => (/[\s'"]/.test(p) ? `'${p.replace(/'/g, "'\\''")}'` : p))
            panes.get(active.id)?.term.paste(quoted.join(' ') + ' ')
          }),
        )
      }
    })()
    return () => offs.forEach((f) => f())
  })

  $effect(() => {
    if (!finding) panes.get(active?.id ?? '')?.search.clearDecorations()
  })
</script>

<svelte:window onkeydowncapture={onkeydown} />

<div
  class="win"
  class:busy={winState === 'busy'}
  class:waiting={winState === 'waiting'}
  class:mac={isMac && inTauri}
>
  <!-- Başlık satırı: Mac'te trafik ışıkları burada (sistem başlık çubuğu yok); sürükleyerek pencere taşınır. -->
  <header class="head" data-tauri-drag-region>
    <span class="hex" class:on={winState === 'busy' || winState === 'running'} data-tauri-drag-region>⬡</span>
    <span class="proj" data-tauri-drag-region>{project}</span>
    <span class="name" data-tauri-drag-region>{headTitle}</span>
    <span class="grow" data-tauri-drag-region></span>
    <span class="badge {winState}" title={chat ? `${chat.tool} · ${STATUS_LABEL[chat.status]}` : 'terminal'}>
      {BADGE[winState]}{elapsed ? ` ${elapsed}` : ''}
    </span>
  </header>

  <!-- Bilgi şeridi: bağlam, model, effort, maliyet, araçlar; sağda ikon butonlar. -->
  <div class="strip">
    <!-- Hep aynı bilgiler: bu terminaldeki sohbet; yoksa aynı klasördeki son sohbet (soluk); o da yoksa boş. -->
    <span class="info" class:stale={!chat} title={info && !chat ? `last chat in this folder: ${info.title}` : undefined}>
      <span class="k">ctx</span>
      <span class="gauge" class:hot={(info?.contextPct ?? 0) >= 80}><i style:width="{Math.min(100, info?.contextPct ?? 0)}%"></i></span>
      <b>{info ? compact(info.contextTokens) : '—'}{info?.contextWindow ? ` / ${compact(info.contextWindow)}` : ''}</b>
      <span class="sep">·</span><span>{info ? prettyModel(info.model) : 'no chat'}</span>
      {#if info?.effort}<span class="sep">·</span><span>{info.effort}</span>{/if}
      <span class="sep">·</span><span class="cost">{info && info.tool === 'claude' ? usd(info.cost) : '$—'}</span>
      <span class="sep">·</span><span>{info?.toolCalls ?? 0} tools</span>
      <span class="sep">·</span><span class:web={info?.webCalls}>{info?.webCalls ?? 0} web</span>
      <span class="sep">·</span><span class:err={info?.toolErrors}>{info?.toolErrors ?? 0} err</span>
    </span>
    {#if !chat && activeSession}
      <span class="sep">·</span><span class="k">{shellName}</span>
      {#if branch}<span class="br">{branch}</span>{/if}
      {#if activeSession.busy && activeSession.fgName}<span class="run">▸ {activeSession.fgName}</span>{/if}
    {/if}
    <span class="grow"></span>
    {#if info}
      <!-- Son 10 dakikanın etkinliği (30 sn'lik dilimlerde araç çağrısı) -->
      <span class="spark" aria-label="activity, last 10 minutes" title="tool calls, last 10 minutes">
        {#each spark as v, k (k)}<i style:height="{v}%"></i>{/each}
      </span>
    {/if}
    {#if chat}
      <button class="ic" onclick={copyLastAnswer} aria-label="copy last answer" title="copy the last answer, including what scrolled off ({keyLabel(shortcuts.combo('term.copyLast'))})">⧉</button>
    {/if}
    <button class="ic" onclick={() => run('term.find')} aria-label="find" title="find ({keyLabel(shortcuts.combo('term.find'))})">⌕</button>
    <button
      class="ic"
      class:on={pinned}
      aria-pressed={pinned}
      aria-label="pin above all apps"
      title="keep this window above all apps"
      onclick={async () => {
        pinned = !pinned
        if (inTauri) await getCurrentWindow().setAlwaysOnTop(pinned)
      }}>⊼</button
    >
    <button class="ic" aria-label="full screen" title="full screen ({keyLabel(shortcuts.combo('term.fullscreen'))})" onclick={() => run('term.fullscreen')}>⛶</button>
    <button class="ic end" aria-label="end session" title="end this tab's session ({keyLabel(shortcuts.combo('term.endSession'))})" onclick={askEnd}>⏻</button>
  </div>

  {#if model.tabs.length > 1}
    <TabBar {model} {titleOf} onclose={(i) => model.closeTab(i) && closeWindow()} ondragpane={onDragPane} />
  {/if}

  <main>
    {#if ready && model.tab}
      {#key model.tab.id}
        <PaneTree pane={model.tab.root} {model} {cwdOf} {isAppShortcut} oncontextmenu={paneMenu} {dropHint}
          onclosepane={(id) => model.closePane(id) && closeWindow()} titleOf={sessionTitle} />
      {/key}
    {/if}

    {#if finding}
      <form
        class="find"
        onsubmit={(e) => {
          e.preventDefault()
          run('term.findNext')
        }}
      >
        <input
          bind:this={findInput}
          bind:value={query}
          placeholder="find"
          aria-label="find in terminal"
          oninput={() => query && panes.get(active?.id ?? '')?.search.findNext(query, { ...searchOpts, incremental: true })}
          onkeydown={(e) => {
            if (e.key === 'Escape') {
              finding = false
              panes.get(active?.id ?? '')?.focus()
            }
          }}
        />
        <button type="button" title="previous" onclick={() => run('term.findPrev')}>↑</button>
        <button type="submit" title="next">↓</button>
        <button type="button" title="close" onclick={() => (finding = false)}>×</button>
      </form>
    {/if}

    {#if note}<div class="note">{note}</div>{/if}

    {#if confirmEnd}
      <div class="modal" role="alertdialog" aria-labelledby="end-title">
        <div class="box">
          <b id="end-title">End this session?</b>
          <p>Still running: {confirmEnd.running.join(', ')}. Processes that ignore hangup (dev servers holding a port) are ended too.</p>
          <div class="actions">
            <button onclick={() => (confirmEnd = null)}>cancel</button>
            <button class="danger" onclick={endNow}>end session</button>
          </div>
        </div>
      </div>
    {/if}
  </main>

  <!-- Son araç çağrıları (bu sohbetin), kategori renginde çipler; en yenisi sağda. -->
  {#if chat}
    <footer class="tools">
      <span class="st {winState}">{BADGE[winState].toLowerCase()}</span>
      <div class="chips">
        {#each recentTools as e, i (i + ':' + e.ts + ':' + e.id)}
          <span
            class="chip"
            class:fail={e.id != null && engine.failed.has(e.id)}
            style:--c="var(--cat-{e.category ?? 'other'})"
            title="{e.tool} {e.target ?? ''}"
          >
            <b>{CAT_LABEL[e.category ?? 'other'] ?? e.category}</b> {chipText(e.target ?? e.tool ?? '')}
          </span>
        {/each}
      </div>
    </footer>
  {/if}
</div>
<ContextMenu />

<style>
  /* Pencere: neredeyse opak (arkadaki yazılar okunmasın), oturum çalışırken kenarı parlar. */
  .win {
    position: relative;
    height: 100%;
    display: flex;
    flex-direction: column;
    background: rgba(var(--term-bg-rgb), 0.96);
    border: 1px solid rgba(var(--accent-rgb), 0.35);
    border-radius: 10px;
    overflow: hidden;
    animation: crt-on 0.32s ease-out;
    transition: border-color 0.3s, box-shadow 0.3s;
  }
  .win.busy {
    border-color: rgba(var(--accent-rgb), 0.85);
    box-shadow: inset 0 0 0 1px rgba(var(--accent-rgb), 0.25), inset 0 0 24px rgba(var(--accent-rgb), 0.12);
  }
  .win.waiting {
    border-color: rgba(var(--warn-rgb), 0.85);
    box-shadow: inset 0 0 24px rgba(var(--warn-rgb), 0.12);
  }
  @keyframes crt-on {
    0% { transform: scale(1, 0.004); filter: brightness(10); opacity: 0.6; }
    40% { transform: scale(1, 0.004); filter: brightness(6); }
    100% { transform: none; filter: none; opacity: 1; }
  }

  .head {
    flex: none;
    height: 32px;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 8px 0 12px;
    background: rgba(var(--panel-rgb), 0.98);
    border-bottom: 1px solid var(--line-soft);
    user-select: none;
    -webkit-user-select: none;
  }
  /* Mac trafik ışıkları için yer */
  .mac .head { padding-left: 82px; }
  .hex {
    flex: none;
    color: var(--accent);
    font-size: 1.25em;
    line-height: 1;
    opacity: 0.8;
  }
  .hex.on { opacity: 1; text-shadow: 0 0 8px rgba(var(--accent-rgb), 0.9); }
  .proj { color: var(--accent); font-weight: 700; white-space: nowrap; font-size: 1.08em; }
  .name { color: var(--hi); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0; font-size: 1.08em; }
  .grow { flex: 1; align-self: stretch; }
  .badge {
    flex: none;
    padding: 1px 8px;
    border: 1px solid var(--line);
    color: var(--dim);
    font-weight: 700;
    letter-spacing: 0.06em;
    font-variant-numeric: tabular-nums;
  }
  .badge.busy, .badge.running {
    color: var(--accent);
    border-color: var(--accent);
    box-shadow: 0 0 10px rgba(var(--accent-rgb), 0.55), inset 0 0 8px rgba(var(--accent-rgb), 0.25);
    text-shadow: 0 0 6px rgba(var(--accent-rgb), 0.8);
  }
  .badge.waiting {
    color: var(--warn);
    border-color: var(--warn);
    box-shadow: 0 0 10px rgba(var(--warn-rgb), 0.55);
    animation: pulse 1.6s ease-in-out infinite;
  }
  .badge.idle { color: var(--cyan); border-color: rgba(var(--cyan-rgb), 0.6); }
  @keyframes pulse { 50% { opacity: 0.6; } }

  .strip {
    flex: none;
    height: 26px;
    font-size: 1.04em;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 6px 0 12px;
    background: rgba(var(--panel-rgb), 0.92);
    border-bottom: 1px solid var(--line);
    color: var(--txt3);
    white-space: nowrap;
    overflow: hidden;
  }
  .strip .k { color: var(--dim); }
  .strip b { color: var(--txt); font-weight: 600; }
  .strip .sep { color: var(--faint); }
  .strip .cost { color: var(--amber); }
  .strip .err { color: var(--red); }
  .strip .run { color: var(--accent); }
  .strip .br { color: var(--magenta); }
  .info { display: flex; align-items: center; gap: 6px; min-width: 0; overflow: hidden; }
  .info.stale { opacity: 0.55; }
  .strip .web { color: var(--cyan); }
  .spark { flex: none; display: flex; align-items: flex-end; gap: 2px; height: 16px; margin-right: 6px; }
  .spark i { width: 3px; background: rgba(var(--accent-rgb), 0.75); }
  .gauge { flex: none; width: 90px; height: 5px; background: var(--line-soft); }
  .gauge i { display: block; height: 100%; background: var(--accent); box-shadow: 0 0 6px rgba(var(--accent-rgb), 0.6); }
  .gauge.hot i { background: var(--red); }
  .ic {
    all: unset;
    flex: none;
    width: 22px;
    height: 18px;
    display: grid;
    place-content: center;
    border: 1px solid var(--line-soft);
    color: var(--txt3);
    cursor: pointer;
  }
  .ic:hover, .ic:focus-visible { color: var(--hi); border-color: var(--accent); }
  .ic.on { color: var(--accent); border-color: var(--accent); }
  .ic.end { color: var(--warn); border-color: rgba(var(--warn-rgb), 0.45); }

  .tools {
    flex: none;
    height: 22px;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 8px;
    background: rgba(var(--panel-rgb), 0.92);
    border-top: 1px solid var(--line);
    overflow: hidden;
  }
  .st { flex: none; color: var(--dim); }
  .st.busy { color: var(--accent); }
  .st.waiting { color: var(--warn); }
  .st.idle { color: var(--cyan); }
  .chips { flex: 1; min-width: 0; display: flex; justify-content: flex-end; gap: 4px; overflow: hidden; }
  .chip {
    flex: none;
    max-width: 130px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    padding: 0 6px;
    border: 1px dashed rgba(var(--accent-rgb), 0.3);
    border-left: 2px solid var(--c);
    color: var(--txt3);
    font-size: 0.9em;
  }
  .chip b { color: var(--c); font-weight: 600; }
  .chip.fail { border-left-color: var(--red); }
  .chip.fail b { color: var(--red); }
  main { position: relative; flex: 1; min-height: 0; display: flex; }

  .find {
    position: absolute;
    top: 6px;
    right: 14px;
    z-index: 10;
    display: flex;
    gap: 2px;
    padding: 3px;
    background: var(--panel);
    border: 1px solid var(--accent);
  }
  .find input {
    all: unset;
    width: 200px;
    padding: 1px 6px;
    color: var(--hi);
    background: rgba(var(--bg-rgb), 0.6);
  }
  .find button { all: unset; padding: 0 6px; color: var(--txt2); cursor: pointer; }
  .find button:hover { color: var(--hi); }

  .note {
    position: absolute;
    right: 14px;
    bottom: 12px;
    z-index: 10;
    padding: 3px 10px;
    background: var(--accent);
    color: var(--ink);
    font-weight: 700;
    pointer-events: none;
  }

  .modal {
    position: absolute;
    inset: 0;
    z-index: 20;
    display: grid;
    place-content: center;
    background: rgba(0, 0, 0, 0.45);
  }
  .box {
    width: min(420px, 90vw);
    display: grid;
    gap: 8px;
    padding: 14px;
    background: var(--panel);
    border: 1px solid var(--warn);
    color: var(--txt2);
  }
  .box b { color: var(--hi); }
  .box p { margin: 0; }
  .actions { display: flex; justify-content: flex-end; gap: 8px; }
  .actions button { all: unset; padding: 2px 10px; border: 1px solid var(--line); cursor: pointer; }
  .actions button:focus-visible, .actions button:hover { border-color: var(--accent); color: var(--hi); }
  .actions .danger { color: var(--ink); background: var(--warn); border-color: var(--warn); }
</style>
