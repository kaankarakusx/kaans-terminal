<script lang="ts">
  import Limits from './Limits.svelte'
  import Marquee from './Marquee.svelte'
  import { engine } from './engine.svelte'
  import { ALL_PANELS, layout } from './layout.svelte'
  import { menu } from './menu.svelte'
  import { notifier } from './notify.svelte'
  import { label, shortcuts } from './shortcuts.svelte'
  import { ui } from './ui.svelte'
  import { now } from './now.svelte'
  import { updates } from './updates.svelte'
  import { clock, compact, pct, usd } from './format'

  const k = $derived(engine.snapshot?.kpis)
  const unpriced = $derived(engine.snapshot?.sessions.some((s) => s.unpriced) ?? false)

  function panelsMenu(e: MouseEvent) {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect()
    menu.show(r.left, r.bottom + 4, [
      ...ALL_PANELS.map((id) => ({ label: id, checked: !layout.isHidden(id), action: () => layout.toggle(id) })),
      'sep',
      { label: 'reset layout', action: () => layout.reset() },
    ])
  }
</script>

<header class="topbar">
  <div class="brand">
    <img src="/logo.svg" alt="" width="26" height="26" />
    <div class="logo"><small>KAAN'S</small><b>TERMINAL</b></div>
  </div>
  <div class="clock">
    <i class="live" class:on={engine.connected}></i>
    {clock(now.value)}
  </div>

  <div class="kpis">
    <div class="kpi"><span class="l">sessions live</span><b>{k?.sessionsLive ?? '—'}</b></div>
    <div class="kpi"><span class="l">agents running</span><b>{k?.agentsRunning ?? '—'}</b></div>
    <div class="kpi"><span class="l">tools/min</span><b>{k ? k.toolsPerMin.toFixed(1) : '—'}</b><span class="s">last 5m</span></div>
    <div class="kpi"><span class="l">tokens/min</span><b>{k ? compact(k.tokensPerMin) : '—'}</b><span class="s">incl. cache</span></div>
    <div
      class="kpi hot"
      title="API list-price equivalent of today's tokens — not your subscription bill.{unpriced
        ? ' Some models are missing from the price table.'
        : ''}"
    >
      <span class="l">today · api-eq</span>
      <b>{k ? usd(k.costToday) : '—'}{unpriced ? '+' : ''}</b>
      <span class="s">{k ? usd(k.burnPerHour) : '—'}/h</span>
    </div>
    <div class="kpi">
      <span class="l">cache hit</span><b>{pct(k?.cacheHitPct ?? null)}</b>
      <span class="s">{k ? usd(k.cacheSavingsToday) : '—'} saved</span>
    </div>
    <div class="kpi"><span class="l">web calls</span><b>{k?.webCalls ?? '—'}</b><span class="s">today</span></div>
    <div class="kpi">
      <span class="l">tool ok</span><b>{pct(k?.toolOkPct ?? null)}</b>
      <span class="s" class:err={(k?.toolErrors ?? 0) > 0}>{k?.toolErrors ?? 0} errors</span>
    </div>
  </div>

  <Limits />
  <Marquee />

  <div class="buttons">
    {#if engine.demo}<span class="demo" title="browser preview with fake data">demo</span>{/if}
    {#if updates.view.status === 'downloading'}
      <button class="topbtn upd" title="downloading {updates.view.version}" onclick={() => ui.openSettings('about')}>updating {updates.view.pct ?? 0}%</button>
    {:else if updates.view.status === 'ready'}
      <button class="topbtn upd ready" title="{updates.view.version} is ready · installs when you're away 10 min · terminals keep running" onclick={() => ui.openSettings('about')}>↻ update {updates.view.version}</button>
    {/if}
    <button class="topbtn primary" title="new terminal ({label(shortcuts.combo('app.newTerminal'))})" onclick={() => (ui.picker = true)}>+ new terminal</button>
    <button class="topbtn" onclick={panelsMenu} aria-haspopup="menu">panels ▾</button>
    <button
      class="topbtn"
      class:on={notifier.sound}
      aria-pressed={notifier.sound}
      title="play a sound when a chat needs you"
      onclick={() => notifier.toggleSound()}>sound {notifier.sound ? 'on' : 'off'}</button
    >
    <button class="topbtn" title="settings · themes, interface, integrations & shortcuts ({label(shortcuts.combo('app.shortcuts'))})" onclick={() => ui.openSettings('integrations')}>⚙</button>
  </div>
</header>

<style>
  .topbar {
    display: flex;
    align-items: center;
    gap: 20px;
    padding: 6px 12px;
    background: var(--panel);
    border: 1px solid var(--line);
    min-width: 0;
  }
  .brand { display: flex; align-items: center; gap: 8px; flex: none; }
  .logo { display: grid; line-height: 1.05; }
  .logo small { color: var(--txt3); letter-spacing: 0.3em; font-size: 0.75em; }
  .logo b {
    color: var(--hi);
    letter-spacing: 0.2em;
    text-shadow: 0 0 10px rgba(var(--accent-rgb), 0.8);
  }
  .clock { display: flex; align-items: center; gap: 6px; color: var(--txt2); flex: none; }
  .live { width: 7px; height: 7px; border-radius: 50%; background: var(--ended); }
  .live.on { background: var(--green); box-shadow: 0 0 6px var(--green); }
  /* Dar pencerede önce KPI şeridi daralır (sağdakiler kırpılır); butonlar her zaman görünür kalır. */
  .kpis {
    display: flex;
    gap: 22px;
    min-width: 0;
    flex: 0 1 auto;
    overflow: hidden;
    mask-image: linear-gradient(90deg, #000 92%, transparent);
  }
  .kpi { display: grid; line-height: 1.15; white-space: nowrap; }
  .l { color: var(--dim); font-size: 0.85em; }
  b { color: var(--hi); font-size: 1.28em; font-weight: 700; }
  .hot b {
    color: var(--amber);
    text-shadow: 0 0 10px rgba(var(--amber-rgb), 0.55);
  }
  .s { color: var(--dim); font-size: 0.74em; }
  .s.err { color: var(--red); }
  .buttons { display: flex; align-items: center; gap: 6px; flex: none; }
  .topbtn {
    all: unset;
    padding: 2px 8px;
    border: 1px solid var(--line);
    color: var(--txt2);
    cursor: pointer;
    white-space: nowrap;
    transition: color 0.12s, border-color 0.12s, box-shadow 0.12s;
  }
  .topbtn:hover, .topbtn:focus-visible {
    color: var(--hi);
    border-color: var(--accent);
    box-shadow: 0 0 10px rgba(var(--accent-rgb), 0.35);
  }
  .topbtn.on { color: var(--accent); }
  .topbtn.upd { color: var(--cyan); border-color: rgba(var(--cyan-rgb), 0.5); }
  .topbtn.upd.ready { color: var(--green); border-color: var(--green); }
  .topbtn.primary { background: var(--accent); color: var(--ink); border-color: var(--accent); font-weight: 700; }
  .topbtn.primary:hover, .topbtn.primary:focus-visible { color: var(--ink); }
  .demo { color: var(--ink); background: var(--amber); padding: 0 6px; font-weight: 700; }
</style>
