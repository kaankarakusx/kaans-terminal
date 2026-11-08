<!--
  HUD paneli: köşe işaretleri, vurgu bloğunda etiket, sağda meta, şeritten ışık geçişi.
  `id` verilirse düzene bağlanır: şeridinden tutulup taşınır, gizle butonu ve sağ tık menüsü (clear / hide) olur.
-->
<script lang="ts">
  import type { Snippet } from 'svelte'
  import { CLEARABLE, layout, type PanelId } from './layout.svelte'
  import { menu, type MenuItem } from './menu.svelte'

  let {
    tag,
    id,
    meta,
    hot = false,
    alert = false,
    onclear,
    children,
  }: {
    tag: string
    id?: PanelId
    meta?: string
    /** your.call: bekleyen var */
    hot?: boolean
    /** files.heat: same file */
    alert?: boolean
    /** Temizlenirken saklanacak taban değerler (toplam sayan paneller için). */
    onclear?: () => Record<string, number>
    children: Snippet
  } = $props()

  const cleared = $derived(id ? layout.clearedAt(id) > 0 : false)

  function contextmenu(e: MouseEvent) {
    if (!id) return
    e.preventDefault()
    const pid = id
    const items: (MenuItem | 'sep')[] = []
    if (CLEARABLE.has(pid)) {
      items.push({ label: 'clear', action: () => layout.clear(pid, onclear?.()) })
      items.push({ label: 'show what was cleared', disabled: !cleared, action: () => layout.unclear(pid) })
      items.push('sep')
    }
    items.push({ label: 'hide', action: () => layout.hide(pid) })
    menu.show(e.clientX, e.clientY, items)
  }

  // Şeritten sürükleme: 4 px'ten fazla hareket edince taşıma başlar (tıklamalar etkilenmez).
  function pointerdown(e: PointerEvent) {
    if (!id || e.button !== 0 || (e.target as HTMLElement).closest('button')) return
    const pid = id
    const sx = e.clientX
    const sy = e.clientY
    let started = false
    const move = (ev: PointerEvent) => {
      if (!started && Math.hypot(ev.clientX - sx, ev.clientY - sy) < 4) return
      started = true
      layout.drag = { id: pid, x: ev.clientX, y: ev.clientY, target: dropTarget(ev.clientX, ev.clientY, pid) }
    }
    const up = () => {
      window.removeEventListener('pointermove', move)
      window.removeEventListener('pointerup', up)
      const d = layout.drag
      layout.drag = null
      if (started && d?.target) layout.move(pid, d.target)
    }
    window.addEventListener('pointermove', move)
    window.addEventListener('pointerup', up)
  }

  /** İmlecin altındaki panelin hangi yarısındaysa onun önüne/arkasına; boş alanın içine. */
  function dropTarget(x: number, y: number, self: PanelId) {
    const el = document.elementFromPoint(x, y) as HTMLElement | null
    const slot = el?.closest<HTMLElement>('[data-slot]')
    const areaEl = el?.closest<HTMLElement>('[data-area]')
    if (!areaEl) return null
    const area = areaEl.dataset.area as 'left' | 'right' | 'bottom'
    if (!slot) return { area, index: layout.s.areas[area].length }
    const target = slot.dataset.slot as PanelId
    if (target === self) return null
    const r = slot.getBoundingClientRect()
    const after = area === 'bottom' ? x > r.left + r.width / 2 : y > r.top + r.height / 2
    const index = layout.s.areas[area].indexOf(target) + (after ? 1 : 0)
    return { area, index }
  }
</script>

<section class="panel" class:hot class:alert oncontextmenu={contextmenu} aria-label={tag}>
  <!-- Taşıma fareyle yapılır; klavyeyle panel menüsü (sağ tık / panels) kullanılır. -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <header onpointerdown={pointerdown} class:movable={!!id}>
    <span class="tag">{tag}</span>
    {#if cleared}<span class="cleared" title="right-click → show what was cleared">cleared</span>{/if}
    {#if meta}<span class="meta">{meta}</span>{/if}
    {#if id}
      <button class="hide" title="hide panel" aria-label="hide {tag}" onclick={() => id && layout.hide(id)}>×</button>
    {/if}
  </header>
  <div class="body">{@render children()}</div>
</section>

<style>
  .panel {
    position: relative;
    display: flex;
    flex-direction: column;
    min-height: 0;
    min-width: 0;
    height: 100%;
    background: var(--panel);
    border: 1px solid var(--line);
    backdrop-filter: blur(14px);
  }
  .panel::before,
  .panel::after {
    content: '';
    position: absolute;
    width: 8px;
    height: 8px;
    border: 1px solid var(--accent);
    pointer-events: none;
  }
  .panel::before { top: -1px; left: -1px; border-right: 0; border-bottom: 0; }
  .panel::after { bottom: -1px; right: -1px; border-left: 0; border-top: 0; }

  header {
    position: relative;
    flex: none;
    height: 19px;
    display: flex;
    align-items: center;
    gap: 8px;
    padding-right: 4px;
    border-bottom: 1px solid var(--line-soft);
    overflow: hidden;
    user-select: none;
  }
  header.movable { cursor: grab; }
  header::after {
    content: '';
    position: absolute;
    inset: 0;
    background: linear-gradient(90deg, transparent, rgba(var(--accent-soft-rgb), 0.18) 18%, transparent 36%);
    transform: translateX(-100%);
    animation: sweep 7s linear infinite;
    pointer-events: none;
  }
  @keyframes sweep {
    0%, 70% { transform: translateX(-100%); }
    100% { transform: translateX(100%); }
  }
  .tag {
    flex: none;
    height: 100%;
    display: flex;
    align-items: center;
    padding: 0 8px;
    background: var(--accent);
    color: var(--ink);
    font-weight: 700;
    letter-spacing: 0.04em;
  }
  .hot .tag { background: var(--warn); }
  .alert .tag { background: var(--red); }
  .alert .meta { color: var(--red); font-weight: 700; }
  .cleared { color: var(--faint); font-size: 0.85em; }
  .meta {
    margin-left: auto;
    color: var(--dim);
    font-size: 0.9em;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .hide {
    all: unset;
    flex: none;
    width: 19px;
    height: 19px;
    display: grid;
    place-content: center;
    color: var(--dim);
    cursor: pointer;
    opacity: 0;
    transition: opacity 0.12s;
  }
  header:hover .hide, .hide:focus-visible { opacity: 1; }
  .hide:hover { color: var(--hi); }
  .body {
    flex: 1;
    min-height: 0;
    overflow: auto;
  }
</style>
