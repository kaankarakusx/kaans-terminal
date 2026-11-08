<!--
  orbit.map paneli: three.js sahnesi + HTML katmanları (başlıklar, bilgi kutusu, minimap, butonlar, açıklama).
  Kontroller: tık = oturumu göster, oturumu sürükle = taşı (bırakınca evine döner), boşluğu sürükle = döndür,
  sağ sürükle = kaydır, tekerlek = yakınlaş, çift tık = sıfırla. 5 dk hareketsizlikte sinematik kamera.
-->
<script lang="ts">
  import { onMount } from 'svelte'
  import Panel from '../Panel.svelte'
  import StatusDot from '../StatusDot.svelte'
  import { engine } from '../engine.svelte'
  import { focus } from '../focus.svelte'
  import { compact, shortModel } from '../format'
  import { now } from '../now.svelte'
  import { power } from '../power.svelte'
  import { STATUS_LABEL } from '../types'
  import { confirm } from '../dialog.svelte'
  import { openChat, openTerminal } from '../terminals'
  import * as api from '../../term/api'
  import type { Category } from '../types'
  import { CATEGORY, STATUS } from './palette'
  import { OrbitScene, type Hit, type StationInfo } from './scene'

  const CINEMATIC_AFTER_MS = 5 * 60_000
  const reducedMotion =
    typeof matchMedia !== 'undefined' && matchMedia('(prefers-reduced-motion: reduce)').matches

  let host = $state<HTMLDivElement>()
  let minimap = $state<HTMLCanvasElement>()
  let scene: OrbitScene | null = null
  let hover = $state<(Hit & { x: number; y: number }) | null>(null)
  let cinematic = $state(false)
  let failed = $state(false)
  /** Kullanıcı sinematik kamerayı kapattıysa, bir sonraki hareketsizliğe kadar yeniden açılmaz. */
  let cineDismissedAt = 0

  const sessions = $derived(engine.snapshot?.sessions ?? [])
  const orbiting = $derived(sessions.filter((s) => s.status === 'busy').length)
  const parked = $derived(sessions.filter((s) => s.status === 'waiting' || s.status === 'idle').length)
  const hovered = $derived(hover?.kind === 'session' ? engine.sessionsById.get(hover.id) : undefined)
  const hoveredCar = $derived(hover?.kind === 'car' ? engine.snapshot?.terminals?.find((t) => t.id === hover?.id) : undefined)
  const hoveredLeftover = $derived(
    hover?.kind === 'leftover' ? engine.snapshot?.leftovers?.find((l) => `lo:${l.pid}` === hover?.id) : undefined,
  )
  const hoveredApp = $derived(hover?.kind === 'app' ? engine.snapshot?.apps?.find((a) => String(a.pid) === hover?.id) : undefined)
  const hoveredMeteor = $derived(
    hover?.kind === 'meteor' ? engine.snapshot?.runaways?.find((r) => String(r.pid) === hover?.id) : undefined,
  )

  function select(hit: Hit) {
    const snap = engine.snapshot
    if (hit.kind === 'session') {
      const s = engine.sessionsById.get(hit.id)
      if (s) void openChat(s)
      else focus.session(hit.id)
    } else if (hit.kind === 'car') {
      void openTerminal(hit.id)
    } else if (hit.kind === 'leftover') {
      const l = snap?.leftovers?.find((x) => `lo:${x.pid}` === hit.id)
      if (!l) return
      confirm.ask({
        title: `End ${l.name}?`,
        body: `Left behind by a closed terminal in ${l.cwd} (pid ${l.pid}). It may be holding a port.`,
        action: 'end process',
        danger: true,
        run: () => api.killLeftover(l.pid),
      })
    } else if (hit.kind === 'app') {
      // Bizim terminalimizdeyse o terminal, değilse onu çalıştıran iTerm/Terminal penceresi öne gelir
      const a = snap?.apps?.find((x) => String(x.pid) === hit.id)
      if (!a) return
      if (a.terminal) void openTerminal(a.terminal, a.label)
      else
        void api.focusExternal(a.pid).then((ok) => {
          // Bağlı bir terminal penceresi yoksa (kapanmış terminalden kalmış): kapatmayı sor
          if (ok) return
          confirm.ask({
            title: `End ${a.label}?`,
            body: `${a.label} in ${a.cwd} (pid ${a.pid}) has no terminal window. It may be holding a port.`,
            action: 'end process',
            danger: true,
            run: () => api.dangerKill(a.pid),
          })
        })
    } else if (hit.kind === 'meteor') {
      const r = snap?.runaways?.find((x) => String(x.pid) === hit.id)
      if (!r) return
      confirm.ask({
        title: `End runaway ${r.name}?`,
        body: `${r.cpuPct}% CPU for 2+ minutes, no terminal, parent gone (pid ${r.pid}${r.path ? `, ${r.path}` : ''}).`,
        action: 'end process',
        danger: true,
        run: () => api.dangerKill(r.pid),
      })
    }
  }

  function stopCinematic() {
    if (!cinematic) return
    cinematic = false
    cineDismissedAt = power.lastInput
    scene?.setCinematic(false)
  }

  onMount(() => {
    if (!host) return
    try {
      scene = new OrbitScene(host, {
        reducedMotion,
        onSelect: select,
        onHover: (h) => (hover = h),
        onUserCamera: () => stopCinematic(),
      })
    } catch (e) {
      console.error('[orbit] WebGL başlatılamadı', e)
      failed = true
      return
    }
    scene.setMinimap(minimap ?? null)
    const offExit = api.onExit((id, ended) => {
      if (ended) scene?.explode(id)
    })
    const off = engine.onEvents((batch) => {
      for (const e of batch) {
        if (e.kind === 'tool') scene?.bolt(e.session, e.category)
        else if (e.kind === 'toolError') scene?.bolt(e.session, e.category, true)
      }
    })
    return () => {
      off()
      void offExit.then((f) => f())
      scene?.dispose()
      scene = null
    }
  })

  $effect(() => {
    // Bitmiş sohbetler haritada da yok (son hali yörüngeden sönerek çıkar)
    scene?.setSessions(
      sessions.filter((s) => s.status !== 'ended').map((s) => ({
        id: s.id,
        title: s.title,
        project: s.project,
        status: s.status,
        hue: s.hue,
        subagents: s.subagents.length,
      })),
    )
  })

  // Terminaller (içinde Claude çalışmayanlar) araba; kapanmış terminallerden kalanlar amber araba.
  $effect(() => {
    const snap = engine.snapshot
    scene?.setCars([
      // Açık terminaller: komut çalışıyorsa komutun adı, hazırsa proje adı (kabuk adı değil). İçinde uygulama
      // çalışan terminal rozet olarak çizilmez: o uygulamanın arabası temsil eder.
      ...(snap?.terminals ?? [])
        .filter((t) => !t.chat && !(snap?.apps ?? []).some((a) => a.terminal === t.id))
        .map((t) => ({ id: t.id, title: t.busy ? t.title : t.project, busy: t.busy })),
      ...(snap?.leftovers ?? []).map((l) => ({ id: `lo:${l.pid}`, title: l.name, busy: true, leftover: true })),
    ])
    scene?.setMeteors((snap?.runaways ?? []).map((r) => ({ pid: r.pid, name: r.name })))
    scene?.setApps((snap?.apps ?? []).map((a) => ({ id: String(a.pid), label: a.label, project: a.project, cpuPct: a.cpuPct })))
  })

  $effect(() => {
    scene?.setPower(power.mode, power.saver)
  })

  // Çekirdek HUD'u: ortada tools/min; halkalar dıştan içe cache hit, Claude 5 sa, hafta, etkinlik
  $effect(() => {
    const k = engine.snapshot?.kpis
    const claude = engine.snapshot?.limits?.find((l) => l.tool === 'claude')
    scene?.setCore({
      value: k ? k.toolsPerMin.toFixed(1) : '0.0',
      label: 'tools/min',
      rings: [
        { value: (k?.cacheHitPct ?? 0) / 100, color: CATEGORY.read, label: 'cache' },
        { value: (claude?.fiveHour?.usedPct ?? 0) / 100, color: STATUS.busy, label: '5h' },
        { value: (claude?.weekly?.usedPct ?? 0) / 100, color: CATEGORY.agent, label: 'week' },
        { value: Math.min(1, (k?.toolsPerMin ?? 0) / 20), color: '#f5ebe0', label: 'load' },
      ],
    })
  })

  // İstasyon etiketleri: bugünkü sayı (tool.mix) ve o kategorideki son hedef
  $effect(() => {
    const mix = engine.snapshot?.kpis?.toolMix ?? {}
    const info: Partial<Record<Category, StationInfo>> = {}
    for (const [cat, count] of Object.entries(mix)) info[cat as Category] = { count: count ?? 0, last: '' }
    for (let i = engine.events.length - 1; i >= 0; i--) {
      const e = engine.events[i]
      if (e.kind !== 'tool' || !e.category) continue
      const slot = (info[e.category] ??= { count: 0, last: '' })
      if (!slot.last && e.target) slot.last = e.target.split('/').filter(Boolean).slice(-2).join('/')
    }
    scene?.setStations(info)
  })

  // Sinematik kamera: 5 dk hareketsizlik; pilde/düşük güçte ya da "hareketi azalt" açıkken kendiliğinden başlamaz.
  $effect(() => {
    const idle = now.value - power.lastInput
    if (cinematic && idle < 1500) {
      stopCinematic()
    } else if (
      !cinematic &&
      idle >= CINEMATIC_AFTER_MS &&
      !power.saver &&
      !reducedMotion &&
      power.lastInput !== cineDismissedAt
    ) {
      cinematic = true
      scene?.setCinematic(true)
    }
  })

  function minimapClick(e: MouseEvent) {
    const c = e.currentTarget as HTMLCanvasElement
    const r = c.getBoundingClientRect()
    const s = c.width / 2 / 14
    const x = ((e.clientX - r.left) * (c.width / r.width) - c.width / 2) / s
    const z = ((e.clientY - r.top) * (c.height / r.height) - c.height / 2) / s
    scene?.lookFrom(x, z)
  }
</script>

<Panel tag="orbit.map" meta="{orbiting} orbiting · {parked} parked">
  <div class="map" bind:this={host}>
    {#if failed}
      <div class="fallback">orbit.map needs WebGL, which is unavailable here</div>
    {/if}

    <!-- Alt satır: durum renkleri ve kontroller -->
    <div class="legend" aria-hidden="true">
      <span style:--c={STATUS.busy}>working</span>
      <span style:--c={STATUS.idle}>your turn</span>
      <span style:--c={STATUS.waiting}>needs you</span>
      <span style:--c="#9d6bff">subagent</span>
      <em>click = open · drag a session = move · drag space = rotate · right-drag = pan · wheel = zoom</em>
    </div>

    <div class="mini">
      <canvas
        class="minimap"
        width="120"
        height="120"
        bind:this={minimap}
        onclick={minimapClick}
        title="top view — click to look from there"
      ></canvas>
      <span>{parked} parked</span>
    </div>

    <div class="controls">
      <button title="zoom in" aria-label="zoom in" onclick={() => scene?.zoom(0.8)}>+</button>
      <button title="zoom out" aria-label="zoom out" onclick={() => scene?.zoom(1.25)}>−</button>
      <button title="reset view" aria-label="reset view" onclick={() => scene?.reset()}>⟲</button>
    </div>

    {#if cinematic}
      <button class="cine" onclick={stopCinematic} title="stop cinematic camera">● cinematic ×</button>
    {/if}

    {#if hover && (hoveredCar || hoveredLeftover || hoveredMeteor || hoveredApp)}
      <div
        class="tip"
        style:left="{hover.x + 14 + 270 > (host?.clientWidth ?? 0) ? hover.x - 274 : hover.x + 14}px"
        style:top="{Math.min(hover.y + 10, (host?.clientHeight ?? 0) - 100)}px"
      >
        {#if hoveredApp}
          <b>{hoveredApp.label}</b>
          <span>{hoveredApp.project} · {Math.round(hoveredApp.cpuPct)}% cpu · pid {hoveredApp.pid}</span>
          <span class="dim">{hoveredApp.terminal ? 'click to open its terminal' : 'click to bring its terminal forward'}</span>
        {:else if hoveredCar}
          <b>{hoveredCar.title}</b>
          <span>{hoveredCar.busy ? 'running' : 'idle'} · {hoveredCar.project}</span>
          <span class="dim">click to open</span>
        {:else if hoveredLeftover}
          <b style:color="var(--amber)">{hoveredLeftover.name} · pid {hoveredLeftover.pid}</b>
          <span>left behind by a closed terminal</span>
          <span class="dim">click to end it</span>
        {:else if hoveredMeteor}
          <b style:color="var(--sky)">DANGER · {hoveredMeteor.name}</b>
          <span>{hoveredMeteor.cpuPct}% CPU · runaway</span>
          <span class="dim">click to end it</span>
        {/if}
      </div>
    {/if}

    {#if hover && hovered}
      <!-- Kenara yakınken imlecin öbür yanına geçer; haritadan taşmaz. -->
      <div
        class="tip"
        style:left="{hover.x + 14 + 270 > (host?.clientWidth ?? 0) ? hover.x - 274 : hover.x + 14}px"
        style:top="{Math.min(hover.y + 10, (host?.clientHeight ?? 0) - 130)}px"
      >
        <b style:color="var(--hue{hovered.hue})">{hovered.title}</b>
        <StatusDot status={hovered.status} label />
        {#if hovered.waitingFor}<span class="w">{hovered.waitingFor}</span>{/if}
        <span>{shortModel(hovered.model)} · {compact(hovered.contextTokens)} ctx</span>
        <span>{hovered.toolCalls} tools · {hovered.subagents.length} agents</span>
        <span class="dim">{STATUS_LABEL[hovered.status]} · {hovered.lastActivity ? Math.round((now.value - hovered.lastActivity) / 1000) : 0}s ago</span>
      </div>
    {/if}
  </div>
</Panel>

<style>
  .map {
    position: relative;
    height: 100%;
    overflow: hidden;
    background: #040406;
  }
  /* İstasyon etiketi: `files.read 248 ■`, altında son hedef */
  .map :global(.station-label) {
    position: absolute;
    left: 0;
    top: 0;
    display: grid;
    justify-items: center;
    gap: 1px;
    font-size: 0.82em;
    white-space: nowrap;
    text-shadow: 0 0 6px #000, 0 0 2px #000;
    transition: opacity 0.2s;
    will-change: transform;
  }
  .map :global(.st-head) { display: flex; align-items: center; gap: 5px; }
  .map :global(.st-head b) { color: var(--c); font-weight: 700; }
  .map :global(.st-count) { color: #f5ebe0; font-weight: 700; }
  .map :global(.st-head i) { width: 7px; height: 7px; background: #f88008; box-shadow: 0 0 5px #f88008; }
  .map :global(.st-last) { max-width: 24ch; overflow: hidden; text-overflow: ellipsis; color: rgba(245, 235, 224, 0.45); }
  .map :global(.station-label.hot .st-head b) { text-shadow: 0 0 8px var(--c); }
  /* Uygulama arabası etiketi: komut, altında proje · cpu */
  .map :global(.app-label) {
    position: absolute;
    left: 0;
    top: 0;
    display: grid;
    justify-items: center;
    font-size: 0.8em;
    white-space: nowrap;
    text-shadow: 0 0 6px #000, 0 0 2px #000;
    transition: opacity 0.2s;
    will-change: transform;
  }
  .map :global(.app-label b) { color: #5bd8ff; font-weight: 700; }
  .map :global(.app-label span) { color: rgba(245, 235, 224, 0.5); }
  /* Canvas sonradan eklenir; HTML katmanları z-index ile üstte kalır. */
  .map :global(.orbit-canvas) {
    position: absolute;
    z-index: 0;
    inset: 0;
    display: block;
    touch-action: none;
    cursor: grab;
  }
  .map :global(.orbit-canvas:active) { cursor: grabbing; }
  .map :global(.orbit-labels) { position: absolute; z-index: 1; inset: 0; pointer-events: none; overflow: hidden; }
  .map :global(.orbit-label) {
    position: absolute;
    left: 0;
    top: 0;
    max-width: 22ch;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 0.85em;
    text-shadow: 0 0 6px #000, 0 0 2px #000;
    transition: opacity 0.2s;
    will-change: transform;
  }
  .fallback { position: absolute; inset: 0; display: grid; place-content: center; color: var(--dim); }

  .legend, .mini, .controls, .cine, .tip, .fallback { z-index: 2; }
  .legend {
    position: absolute;
    bottom: 8px;
    left: 10px;
    right: 150px;
    display: flex;
    gap: 12px;
    align-items: center;
    font-size: 0.82em;
    color: #b39a82;
    white-space: nowrap;
    overflow: hidden;
    pointer-events: none;
  }
  .legend em { font-style: normal; color: rgba(245, 235, 224, 0.32); overflow: hidden; text-overflow: ellipsis; }
  .legend span::before {
    content: '⬡ ';
    color: var(--c);
    text-shadow: 0 0 6px var(--c);
  }

  .mini {
    position: absolute;
    right: 10px;
    bottom: 10px;
    display: grid;
    gap: 2px;
  }
  .minimap {
    width: 104px;
    height: 104px;
    border: 1px solid rgba(255, 176, 87, 0.3);
    cursor: crosshair;
  }
  .mini span { color: #b39a82; font-size: 0.8em; }

  .controls {
    position: absolute;
    right: 124px;
    bottom: 26px;
    display: grid;
    gap: 4px;
  }
  .controls button, .cine {
    all: unset;
    width: 22px;
    height: 22px;
    display: grid;
    place-content: center;
    color: #f5ebe0;
    background: rgba(3, 2, 8, 0.7);
    border: 1px solid rgba(255, 176, 87, 0.35);
    cursor: pointer;
  }
  .controls button:hover, .controls button:focus-visible, .cine:hover {
    border-color: #f88008;
    box-shadow: 0 0 8px rgba(248, 128, 8, 0.5);
  }
  .cine {
    position: absolute;
    top: 8px;
    right: 10px;
    width: auto;
    padding: 0 8px;
    color: #ffb057;
    animation: cine-pulse 2.4s ease-in-out infinite;
  }
  @keyframes cine-pulse { 50% { box-shadow: 0 0 14px rgba(248, 128, 8, 0.6); } }

  .tip {
    position: absolute;
    z-index: 3;
    display: grid;
    gap: 2px;
    max-width: 260px;
    padding: 6px 8px;
    background: var(--panel);
    border: 1px solid var(--line);
    color: var(--txt2);
    pointer-events: none;
    font-size: 0.9em;
  }
  .tip b { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tip .w { color: var(--warn); }
  .tip .dim { color: var(--dim); }
</style>
