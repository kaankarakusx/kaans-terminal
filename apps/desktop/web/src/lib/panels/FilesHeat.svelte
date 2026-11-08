<!--
  files.heat: en çok dokunulan dosyalar, ısı şeridiyle. İki farklı sohbet son 20 dakikada aynı dosyaya yazdıysa
  şerit ve panel etiketi kırmızıya döner (same file).
-->
<script lang="ts">
  import Panel from '../Panel.svelte'
  import { engine } from '../engine.svelte'
  import { layout } from '../layout.svelte'

  const ID = 'files.heat'

  // Temizlendiyse yalnızca o andan sonraki dokunuşlar.
  const rows = $derived.by(() => {
    const base = layout.baseline(ID)
    return (engine.snapshot?.files ?? [])
      // Taban değer (temizlendiği an) şimdikinden büyük olabilir (gün değişti, engine yeniden kuruldu): sıfırın altına inmez.
      .map((f) => ({ ...f, touches: Math.max(0, f.reads + f.writes - (base[f.path] ?? 0)) }))
      .filter((f) => f.touches > 0 || f.clash)
  })
  const max = $derived(Math.max(1, ...rows.map((r) => r.touches)))
  const clashes = $derived(engine.snapshot?.clashes ?? [])

  function name(path: string) {
    const i = path.lastIndexOf('/')
    return i >= 0 ? path.slice(i + 1) : path
  }
</script>

<Panel
  tag="files.heat"
  id={ID}
  alert={clashes.length > 0}
  meta={clashes.length ? `⚠ same file ×${clashes.length}` : `${rows.length} files`}
  onclear={() => Object.fromEntries((engine.snapshot?.files ?? []).map((f) => [f.path, f.reads + f.writes]))}
>
  {#each rows as f (f.path)}
    <div class="row" class:clash={f.clash} title="{f.path}
{f.reads} reads · {f.writes} writes · {f.sessions.length} chat{f.sessions.length === 1 ? '' : 's'}">
      <span class="p"><b>{name(f.path)}</b> <span class="dir">{f.path.slice(0, -name(f.path).length)}</span></span>
      <span class="bar"><i style:width="{(100 * f.touches) / max}%"></i></span>
      <span class="n">{f.touches}</span>
    </div>
  {:else}
    <div class="empty">no files touched yet</div>
  {/each}
</Panel>

<style>
  .row {
    display: grid;
    grid-template-columns: 1fr 34% auto;
    align-items: center;
    gap: 8px;
    padding: 2px 8px;
    white-space: nowrap;
  }
  .row:hover { background: rgba(var(--accent-rgb), 0.06); }
  .p { overflow: hidden; text-overflow: ellipsis; direction: ltr; }
  .p b { color: var(--txt); font-weight: 600; }
  .dir { color: var(--faint); font-size: 0.9em; }
  .bar { height: 5px; background: var(--line-soft); }
  .bar i {
    display: block;
    height: 100%;
    background: linear-gradient(90deg, rgba(var(--amber-rgb), 0.5), var(--accent));
  }
  .n { color: var(--txt3); min-width: 2ch; text-align: right; }
  .clash .p b, .clash .n { color: var(--red); }
  .clash .bar i { background: var(--red); box-shadow: 0 0 6px rgba(var(--red-rgb), 0.7); }
  .empty { color: var(--dim); padding: 12px; }
</style>
