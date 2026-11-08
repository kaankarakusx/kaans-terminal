<!--
  Tema sayfası (OZELLIKLER.md §5): ana alanı kaplar. Her kart uygulamanın o temadaki küçük bir maketi (üst bar,
  paneller, terminalde git status, 16 ANSI rengi). Üzerine gelince bütün uygulama (açık terminal pencereleri dahil)
  o temada görünür, tıklayınca seçilir. Esc ya da palet ikonu kapatır. Kartlar temanın kendi renkleriyle çizilir
  (satır içi renkler tema dosyalarından gelir).
-->
<script lang="ts">
  import { onDestroy } from 'svelte'
  import { deriveTheme } from '../themes/derive'
  import { themes } from '../themes/store.svelte'
  import { ui } from './ui.svelte'

  const cards = themes.all.map((t) => ({ t, d: deriveTheme(t) }))

  onDestroy(() => themes.setPreview(null))

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') ui.themes = false
  }
</script>

<svelte:window {onkeydown} />

<section class="page" aria-label="themes">
  <header>
    <span class="label">themes</span>
    <span class="grow"></span>
    <span class="meta">{themes.all.length} themes · point to try · click to keep · the orbit map keeps its colours</span>
    <button class="close" title="close (Esc)" aria-label="close themes" onclick={() => (ui.themes = false)}>×</button>
  </header>

  <div class="grid" role="listbox" aria-label="themes" tabindex="-1" onmouseleave={() => themes.setPreview(null)}>
    {#each cards as { t, d } (t.id)}
      {@const on = themes.id === t.id}
      <button
        class="card"
        class:on
        class:sig={!!t.signature}
        role="option"
        aria-selected={on}
        style:--m-bg={d['--bg']}
        style:--m-panel="rgb({d['--panel-rgb']})"
        style:--m-term={t.bg}
        style:--m-fg={t.fg}
        style:--m-dim={d['--dim']}
        style:--m-acc={t.accent}
        style:--m-ink={d['--ink']}
        onmouseenter={() => themes.setPreview(t.id)}
        onfocus={() => themes.setPreview(t.id)}
        onblur={() => themes.setPreview(null)}
        onclick={() => themes.select(t.id)}
      >
        <span class="mock" aria-hidden="true">
          <span class="mbar"><b>KAAN'S</b><i></i></span>
          <span class="mbody">
            <span class="mside">
              <i style:border-left-color={d['--cyan']}></i>
              <i style:border-left-color={d['--green']}></i>
              <i style:border-left-color={d['--violet']}></i>
            </span>
            <span class="mterm">
              <span><em class="acc">❯</em> git status</span>
              <span class="ind" style:color={t.ansi[1]}>modified: app.js</span>
              <span class="ind" style:color={t.ansi[2]}>new file: themes.json</span>
              <span><em style:color={t.ansi[6]}>~/repo</em> <em style:color={t.ansi[4]}>main</em> <i class="cur"></i></span>
            </span>
          </span>
          <span class="sw">{#each t.ansi as c, i (i)}<i style:background={c}></i>{/each}</span>
        </span>
        <span class="name">{t.name}{#if on}<small>active</small>{/if}</span>
      </button>
    {/each}
  </div>
</section>

<style>
  .page {
    height: 100%;
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--bg);
    border: 1px solid var(--line-soft);
    animation: fade 0.16s ease-out;
  }
  @keyframes fade { from { opacity: 0; } }
  header { display: flex; align-items: center; gap: 10px; height: 22px; border-bottom: 1px solid var(--line-soft); }
  .label { height: 100%; display: flex; align-items: center; padding: 0 8px; background: var(--accent); color: var(--ink); font-weight: 700; letter-spacing: 0.04em; }
  .grow { flex: 1; }
  .meta { color: var(--dim); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0; }
  .close { all: unset; padding: 0 10px; color: var(--dim); cursor: pointer; }
  .close:hover, .close:focus-visible { color: var(--hi); }

  .grid {
    flex: 1;
    overflow: auto;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
    align-content: start;
    gap: 18px 16px;
    padding: 16px;
  }
  .card { all: unset; display: flex; flex-direction: column; gap: 7px; cursor: pointer; min-width: 0; }
  .mock {
    display: flex;
    flex-direction: column;
    gap: 5px;
    aspect-ratio: 16 / 7.4;
    padding: 5px;
    background: var(--m-bg);
    border: 1px solid color-mix(in oklch, var(--m-acc) 35%, transparent);
    font-size: 0.9em;
    overflow: hidden;
    transition: transform 0.14s, box-shadow 0.14s, border-color 0.14s;
  }
  .mbar { display: flex; align-items: center; gap: 6px; flex: none; }
  .mbar b { padding: 0 5px; background: var(--m-acc); color: var(--m-ink); font-size: 0.75em; letter-spacing: 0.08em; }
  .mbar i { flex: 1; height: 1px; background: color-mix(in oklch, var(--m-acc) 45%, transparent); }
  .mbody { flex: 1; min-height: 0; display: grid; grid-template-columns: 1fr 2.3fr; gap: 5px; }
  .mside { display: grid; grid-template-rows: repeat(3, 1fr); gap: 4px; }
  .mside i { background: var(--m-panel); border: 1px solid color-mix(in oklch, var(--m-acc) 18%, transparent); border-left-width: 2px; }
  .mterm {
    display: flex;
    flex-direction: column;
    gap: 1px;
    padding: 4px 6px;
    background: var(--m-term);
    border: 1px solid color-mix(in oklch, var(--m-acc) 25%, transparent);
    color: var(--m-fg);
    white-space: nowrap;
    overflow: hidden;
  }
  .mterm em { font-style: normal; }
  .mterm .acc { color: var(--m-acc); }
  .ind { padding-left: 1.2em; }
  .cur { display: inline-block; width: 0.55em; height: 1em; vertical-align: -0.15em; background: var(--m-acc); }
  .sw { display: flex; gap: 2px; flex: none; }
  .sw i { flex: 1; height: 7px; }
  /* İmza tema kartı: çizgili, eğik kesimli etiket ve parlayan köşe */
  .card.sig .mbar b {
    padding-right: 10px;
    background: repeating-linear-gradient(135deg, transparent 0 4px, rgba(0, 0, 0, 0.2) 4px 6px), var(--m-acc);
    clip-path: polygon(0 0, 100% 0, calc(100% - 6px) 100%, 0 100%);
  }
  .card.sig .mock { box-shadow: inset 2px 2px 0 -1px var(--m-acc), inset -2px -2px 0 -1px var(--m-acc); }
  .name { color: var(--txt2); font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .name small { margin-left: 8px; color: var(--accent); font-size: 0.8em; letter-spacing: 0.08em; text-transform: uppercase; }

  .card:hover .mock, .card:focus-visible .mock {
    transform: translateY(-2px);
    border-color: var(--m-acc);
    box-shadow: 0 8px 22px color-mix(in oklch, var(--m-acc) 28%, transparent);
  }
  .card:hover .name, .card:focus-visible .name { color: var(--hi); }
  .card.on .mock { border-color: var(--m-acc); box-shadow: 0 0 0 1px var(--m-acc), 0 0 22px color-mix(in oklch, var(--m-acc) 40%, transparent); }
  .card.on .name { color: var(--hi); }
  @media (prefers-reduced-motion: reduce) {
    .page { animation: none; }
    .mock { transition: none; }
    .card:hover .mock { transform: none; }
  }
</style>
