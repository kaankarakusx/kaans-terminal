<!--
  Bir düzen alanı (sol / sağ / alt): görünür panelleri ağırlıklarıyla dizer, aralarındaki çizgiden
  boyutlandırır, alanın iç kenarından alanı büyütüp küçültür ve taşınan panel için bırakma yerini gösterir.
-->
<script lang="ts">
  import { layout, type Area, type PanelId } from './layout.svelte'
  import { PANELS } from './panels'

  let { area }: { area: Area } = $props()

  const ids = $derived(layout.visible(area))
  const horizontal = $derived(area === 'bottom')
  let el = $state<HTMLDivElement>()

  /** Bırakma çizgisinin hangi panelin önünde/arkasında görüneceği. */
  const drop = $derived.by(() => {
    const t = layout.drag?.target
    if (!t || t.area !== area) return null
    const all = layout.s.areas[area]
    const before = all.slice(t.index).find((id) => ids.includes(id))
    return before ? { id: before, side: 'before' as const } : { id: ids.at(-1), side: 'after' as const }
  })

  function dragLine(e: PointerEvent, onmove: (ev: PointerEvent) => void, ondone: () => void) {
    e.preventDefault()
    const target = e.currentTarget as HTMLElement
    target.setPointerCapture(e.pointerId)
    const move = (ev: PointerEvent) => onmove(ev)
    const up = () => {
      target.removeEventListener('pointermove', move)
      target.removeEventListener('pointerup', up)
      ondone()
    }
    target.addEventListener('pointermove', move)
    target.addEventListener('pointerup', up)
  }

  // İki komşu panel arasındaki çizgi.
  function split(e: PointerEvent, a: PanelId, b: PanelId) {
    const slots = el?.querySelectorAll<HTMLElement>('[data-slot]')
    const ra = [...(slots ?? [])].find((s) => s.dataset.slot === a)?.getBoundingClientRect()
    const rb = [...(slots ?? [])].find((s) => s.dataset.slot === b)?.getBoundingClientRect()
    if (!ra || !rb) return
    const sizeA = horizontal ? ra.width : ra.height
    const sizeB = horizontal ? rb.width : rb.height
    const total = layout.weight(a) + layout.weight(b)
    const start = horizontal ? e.clientX : e.clientY
    dragLine(
      e,
      (ev) => {
        const d = (horizontal ? ev.clientX : ev.clientY) - start
        const wa = ((sizeA + d) / (sizeA + sizeB)) * total
        layout.setWeights(a, wa, b, total - wa)
      },
      () => layout.save(),
    )
  }

  // Alanın iç kenarı (sol alanın sağı, sağ alanın solu, alt alanın üstü).
  function edge(e: PointerEvent) {
    const app = el?.closest('.app')?.getBoundingClientRect()
    if (!app) return
    dragLine(
      e,
      (ev) => {
        const pct =
          area === 'left'
            ? ((ev.clientX - app.left) / app.width) * 100
            : area === 'right'
              ? ((app.right - ev.clientX) / app.width) * 100
              : ((app.bottom - ev.clientY) / app.height) * 100
        layout.setSize(area, pct)
      },
      () => layout.save(),
    )
  }
</script>

<div class="area {area}" class:horizontal data-area={area} bind:this={el}>
  {#each ids as id, i (id)}
    {#if i > 0}
      <div
        class="split"
        role="separator"
        aria-orientation={horizontal ? 'vertical' : 'horizontal'}
        onpointerdown={(e) => split(e, ids[i - 1], id)}
      ></div>
    {/if}
    {@const Comp = PANELS[id]}
    <div
      class="slot"
      data-slot={id}
      style:flex-grow={layout.weight(id)}
      class:drop-before={drop?.id === id && drop.side === 'before'}
      class:drop-after={drop?.id === id && drop.side === 'after'}
      class:dragging={layout.drag?.id === id}
    >
      <Comp />
    </div>
  {/each}
  {#if ids.length === 0 && layout.drag}
    <div class="empty" class:drop-here={layout.drag.target?.area === area}>drop here</div>
  {/if}
  {#if ids.length > 0}
    <div class="edge" role="separator" onpointerdown={edge}></div>
  {/if}
</div>

<style>
  .area {
    position: relative;
    min-width: 0;
    min-height: 0;
    height: 100%;
    display: flex;
    flex-direction: column;
  }
  .area.horizontal { flex-direction: row; }
  .slot {
    position: relative;
    flex-basis: 0;
    flex-shrink: 1;
    min-width: 0;
    min-height: 0;
    transition: opacity 0.15s;
  }
  .slot.dragging { opacity: 0.35; }
  .split { flex: 0 0 8px; cursor: row-resize; }
  .horizontal > .split { cursor: col-resize; }
  .split:hover { background: rgba(var(--accent-rgb), 0.12); }

  .drop-before::before, .drop-after::after {
    content: '';
    position: absolute;
    z-index: 5;
    left: 0;
    right: 0;
    height: 2px;
    background: var(--accent);
    box-shadow: 0 0 8px var(--accent);
  }
  .drop-before::before { top: -5px; }
  .drop-after::after { bottom: -5px; }
  .horizontal .drop-before::before, .horizontal .drop-after::after {
    top: 0;
    bottom: 0;
    width: 2px;
    height: auto;
  }
  .horizontal .drop-before::before { left: -5px; right: auto; }
  .horizontal .drop-after::after { right: -5px; left: auto; }

  .empty {
    flex: 1;
    min-width: 60px;
    display: grid;
    place-content: center;
    border: 1px dashed var(--line);
    color: var(--dim);
  }
  .empty.drop-here { border-color: var(--accent); color: var(--accent); }

  .edge { position: absolute; z-index: 4; }
  .left > .edge { top: 0; bottom: 0; right: -8px; width: 8px; cursor: col-resize; }
  .right > .edge { top: 0; bottom: 0; left: -8px; width: 8px; cursor: col-resize; }
  .bottom > .edge { left: 0; right: 0; top: -8px; height: 8px; cursor: row-resize; }
  .edge:hover { background: rgba(var(--accent-rgb), 0.12); }
</style>
