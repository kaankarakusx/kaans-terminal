<!--
  Sekmeler: eşit genişlik, uzun ad "…". Çubukta sürüklemek sırayı değiştirir; bir bölmenin üstüne bırakmak
  o bölmeyi böler (sol, sağ, üst ya da alt yarı).
-->
<script lang="ts">
  import type { Side, WindowModel } from './model.svelte'

  let {
    model,
    titleOf,
    onclose,
    ondragpane,
  }: {
    model: WindowModel
    titleOf: (tabIndex: number) => { title: string; busy: boolean; chat: boolean }
    onclose: (i: number) => void
    /** Sekme bir bölmenin üstünde; null = değil. Bırakınca `drop` true. */
    ondragpane: (hint: { pane: string; side: Side } | null, drop: boolean, tab: number) => void
  } = $props()

  let bar = $state<HTMLDivElement>()
  let dragging = $state<number | null>(null)
  let over = $state<number | null>(null)

  function paneHint(x: number, y: number): { pane: string; side: Side } | null {
    const el = document.elementFromPoint(x, y)?.closest<HTMLElement>('[data-pane]')
    if (!el) return null
    const r = el.getBoundingClientRect()
    const dx = (x - r.left) / r.width - 0.5
    const dy = (y - r.top) / r.height - 0.5
    const side: Side = Math.abs(dx) > Math.abs(dy) ? (dx < 0 ? 'left' : 'right') : dy < 0 ? 'top' : 'bottom'
    return { pane: el.dataset.pane!, side }
  }

  function down(e: PointerEvent, i: number) {
    if (e.button !== 0 || (e.target as HTMLElement).closest('.x')) return
    const sx = e.clientX
    const sy = e.clientY
    let started = false
    const move = (ev: PointerEvent) => {
      if (!started && Math.hypot(ev.clientX - sx, ev.clientY - sy) < 5) return
      started = true
      dragging = i
      const r = bar?.getBoundingClientRect()
      if (r && ev.clientY >= r.top && ev.clientY <= r.bottom) {
        const w = r.width / Math.max(1, model.tabs.length)
        over = Math.max(0, Math.min(model.tabs.length - 1, Math.floor((ev.clientX - r.left) / w)))
        ondragpane(null, false, i)
      } else {
        over = null
        ondragpane(paneHint(ev.clientX, ev.clientY), false, i)
      }
    }
    const up = (ev: PointerEvent) => {
      window.removeEventListener('pointermove', move)
      window.removeEventListener('pointerup', up)
      if (started) {
        if (over !== null) model.moveTab(i, over)
        else ondragpane(paneHint(ev.clientX, ev.clientY), true, i)
      } else {
        model.selectTab(i)
      }
      dragging = null
      over = null
    }
    window.addEventListener('pointermove', move)
    window.addEventListener('pointerup', up)
  }
</script>

<div class="tabs" bind:this={bar} role="tablist">
  {#each model.tabs as t, i (t.id)}
    {@const info = titleOf(i)}
    <div
      class="tab"
      class:on={i === model.activeTab}
      class:dragging={dragging === i}
      class:over={over === i && dragging !== i}
      role="tab"
      tabindex="0"
      aria-selected={i === model.activeTab}
      title={info.title}
      onpointerdown={(e) => down(e, i)}
      onauxclick={(e) => e.button === 1 && onclose(i)}
      onkeydown={(e) => e.key === 'Enter' && model.selectTab(i)}
    >
      <i class:busy={info.busy} class:chat={info.chat}></i>
      <span class="name">{info.title}</span>
      {#if i < 9}<span class="key">⌘{i + 1}</span>{/if}
      <button class="x" aria-label="close tab (session keeps running)" title="close tab — keeps running" onclick={(e) => { e.stopPropagation(); onclose(i) }}>✕</button>
    </div>
  {/each}
</div>

<style>
  .tabs {
    display: flex;
    height: 26px;
    background: var(--panel);
    border-bottom: 1px solid var(--line);
    user-select: none;
  }
  .tab {
    flex: 1 1 0;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 6px 0 10px;
    color: var(--dim);
    border-right: 1px solid var(--line-soft);
    cursor: default;
  }
  .tab:hover { color: var(--txt2); }
  .tab.on { color: var(--hi); background: rgba(var(--accent-rgb), 0.1); box-shadow: inset 0 -2px 0 var(--accent); }
  .tab.dragging { opacity: 0.4; }
  .tab.over { box-shadow: inset 2px 0 0 var(--accent); }
  .tab:focus-visible { outline: 1px solid var(--accent); outline-offset: -1px; }
  i { width: 6px; height: 6px; border-radius: 50%; background: var(--ended); flex: none; }
  i.busy { background: var(--accent); box-shadow: 0 0 6px var(--accent); }
  i.chat { border-radius: 0; transform: rotate(45deg); }
  .name { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .key { color: var(--faint); font-size: 0.85em; }
  /* Kapatma düğmesi her zaman görünür (VS Code gibi); orta tık da kapatır. */
  .x {
    all: unset;
    flex: none;
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    border-radius: 3px;
    font-size: 11px;
    line-height: 1;
    color: var(--dim);
    opacity: 0.7;
    cursor: pointer;
  }
  .tab.on .x, .tab:hover .x, .x:focus-visible { opacity: 1; }
  .x:hover { color: var(--hi); background: rgba(var(--accent-rgb), 0.18); }
  .x:focus-visible { outline: 1px solid var(--accent); }
</style>
