<!-- Bölme ağacı: yapraklar terminal, bölmeler aralarında sürüklenebilir çizgiyle iki çocuk. -->
<script lang="ts">
  import PaneTree from './PaneTree.svelte'
  import TerminalPane from './TerminalPane.svelte'
  import { leaves, type Pane, type Side, type WindowModel } from './model.svelte'

  let {
    pane,
    model,
    cwdOf,
    isAppShortcut,
    oncontextmenu,
    dropHint,
    onclosepane,
    titleOf,
  }: {
    pane: Pane
    model: WindowModel
    cwdOf: (session: string) => string | null
    isAppShortcut: (e: KeyboardEvent) => boolean
    oncontextmenu: (e: MouseEvent, paneId: string) => void
    /** Sekme bir bölmenin üstünde sürüklenirken: hangi bölme, hangi yarı */
    dropHint: { pane: string; side: Side } | null
    /** Bölmeyi kapat (oturum yaşar) */
    onclosepane: (paneId: string) => void
    /** Oturumun adı ve durumu (sekme çubuğuyla aynı) */
    titleOf: (session: string) => { title: string; busy: boolean; chat: boolean }
  } = $props()

  // Sekmede birden fazla bölme varsa her bölmenin üstünde başlık şeridi (numara, ad, ✕).
  const split = $derived(model.tab?.root.kind === 'split')
  const num = $derived(
    split && model.tab && pane.kind === 'leaf' ? leaves(model.tab.root).findIndex((l) => l.id === pane.id) + 1 : 0,
  )
  const info = $derived(split && pane.kind === 'leaf' ? titleOf(pane.session) : null)

  let box = $state<HTMLDivElement>()

  function drag(e: PointerEvent) {
    if (pane.kind !== 'split' || !box) return
    e.preventDefault()
    const el = e.currentTarget as HTMLElement
    el.setPointerCapture(e.pointerId)
    const r = box.getBoundingClientRect()
    const id = pane.id
    const dir = pane.dir
    const move = (ev: PointerEvent) =>
      model.setRatio(id, dir === 'row' ? (ev.clientX - r.left) / r.width : (ev.clientY - r.top) / r.height)
    const up = () => {
      el.removeEventListener('pointermove', move)
      el.removeEventListener('pointerup', up)
      model.save()
    }
    el.addEventListener('pointermove', move)
    el.addEventListener('pointerup', up)
  }
</script>

{#if pane.kind === 'leaf'}
  <div class="leaf" class:split data-pane={pane.id}>
    {#if split && info}
      <div class="head" class:on={model.tab?.active === pane.id} title={info.title}>
        <button class="pick" onclick={() => model.focusPane(pane.id)}>
          <span class="n">{num}</span>
          <i class:busy={info.busy} class:chat={info.chat}></i>
          <span class="name">{info.title}</span>
        </button>
        <button
          class="x"
          aria-label="close pane (session keeps running)"
          title="close pane — keeps running"
          onclick={() => onclosepane(pane.id)}>✕</button
        >
      </div>
    {/if}
    <div class="body">
      <TerminalPane
        paneId={pane.id}
        session={pane.session}
        cwd={cwdOf(pane.session)}
        active={model.tab?.active === pane.id}
        onfocus={() => model.focusPane(pane.id)}
        {isAppShortcut}
        oncontextmenu={(e) => oncontextmenu(e, pane.id)}
      />
    </div>
    {#if dropHint?.pane === pane.id}
      <div class="drop {dropHint.side}"></div>
    {/if}
  </div>
{:else}
  <div class="split {pane.dir}" bind:this={box}>
    <div class="child" style:flex-grow={pane.ratio}>
      <PaneTree pane={pane.a} {model} {cwdOf} {isAppShortcut} {oncontextmenu} {dropHint} {onclosepane} {titleOf} />
    </div>
    <div
      class="bar"
      role="separator"
      aria-orientation={pane.dir === 'row' ? 'vertical' : 'horizontal'}
      onpointerdown={drag}
    ></div>
    <div class="child" style:flex-grow={1 - pane.ratio}>
      <PaneTree pane={pane.b} {model} {cwdOf} {isAppShortcut} {oncontextmenu} {dropHint} {onclosepane} {titleOf} />
    </div>
  </div>
{/if}

<style>
  .leaf { position: relative; height: 100%; width: 100%; min-width: 0; min-height: 0; }
  .split { display: flex; height: 100%; width: 100%; min-width: 0; min-height: 0; }
  .split.col { flex-direction: column; }
  .child { flex-basis: 0; min-width: 0; min-height: 0; display: flex; }
  .bar { flex: 0 0 4px; background: var(--line-soft); cursor: col-resize; }
  .col > .bar { cursor: row-resize; }
  .bar:hover { background: var(--accent); }
  .leaf.split { display: flex; flex-direction: column; }
  .body { position: relative; flex: 1; min-height: 0; height: 100%; }
  .leaf.split .body { height: auto; }
  /* Bölme başlığı: sekme gibi; öndeki bölme vurgulu. */
  .head {
    flex: none;
    display: flex;
    align-items: center;
    height: 24px;
    padding-right: 4px;
    background: var(--panel);
    border-bottom: 1px solid var(--line);
    color: var(--dim);
    user-select: none;
  }
  .head.on { color: var(--hi); background: rgba(var(--accent-rgb), 0.1); box-shadow: inset 0 -2px 0 var(--accent); }
  .pick {
    all: unset;
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 7px;
    height: 100%;
    padding: 0 8px 0 10px;
    cursor: default;
  }
  .pick:focus-visible { outline: 1px solid var(--accent); outline-offset: -1px; }
  .n { color: var(--faint); font-variant-numeric: tabular-nums; }
  .head.on .n { color: var(--accent); }
  i { width: 6px; height: 6px; border-radius: 50%; background: var(--ended); flex: none; }
  i.busy { background: var(--accent); box-shadow: 0 0 6px var(--accent); }
  i.chat { border-radius: 0; transform: rotate(45deg); }
  .name { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
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
    opacity: 0.7;
    cursor: pointer;
  }
  .head.on .x, .head:hover .x, .x:focus-visible { opacity: 1; }
  .x:hover { color: var(--hi); background: rgba(var(--accent-rgb), 0.2); }
  .x:focus-visible { outline: 1px solid var(--accent); }
  .drop {
    position: absolute;
    z-index: 5;
    background: rgba(var(--accent-rgb), 0.18);
    border: 1px solid var(--accent);
    pointer-events: none;
  }
  .drop.left { left: 0; top: 0; bottom: 0; width: 50%; }
  .drop.right { right: 0; top: 0; bottom: 0; width: 50%; }
  .drop.top { left: 0; right: 0; top: 0; height: 50%; }
  .drop.bottom { left: 0; right: 0; bottom: 0; height: 50%; }
</style>
