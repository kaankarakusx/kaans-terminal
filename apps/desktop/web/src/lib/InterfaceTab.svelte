<!-- INTERFACE (OZELLIKLER.md §9): yazı tipi ve yazı boyutu. Bütün pencerelere anında uygulanır. -->
<script lang="ts">
  import { FONTS, SCALE_MAX, SCALE_MIN, themes, type UiFont } from '../themes/store.svelte'

  const LABEL: Record<UiFont, string> = { mono: 'mono', system: 'system', rounded: 'rounded' }
</script>

<h3>font</h3>
<div class="opts" role="radiogroup" aria-label="font">
  {#each Object.keys(FONTS) as UiFont[] as f (f)}
    <label class="opt" class:on={themes.font === f} style:font-family={FONTS[f]}>
      <input type="radio" name="ui-font" checked={themes.font === f} onchange={() => themes.setFont(f)} />
      {LABEL[f]} <span class="sample">Aa 0123</span>
    </label>
  {/each}
</div>
<p class="note">Terminals always use a monospace font.</p>

<h3>text size · {Math.round(themes.scale * 100)}%</h3>
<input
  class="range"
  type="range"
  min={SCALE_MIN}
  max={SCALE_MAX}
  step="0.05"
  value={themes.scale}
  aria-label="text size"
  oninput={(e) => themes.setScale(Number(e.currentTarget.value))}
/>
<div class="ticks"><span>100%</span><span>140%</span></div>

<style>
  h3 { margin: 10px 0 6px; color: var(--dim); font-size: 0.85em; font-weight: 600; text-transform: uppercase; letter-spacing: 0.05em; }
  .opts { display: flex; gap: 8px; flex-wrap: wrap; }
  .opt { display: flex; align-items: center; gap: 6px; padding: 4px 10px; border: 1px solid var(--line); color: var(--txt2); cursor: pointer; }
  .opt.on { border-color: var(--accent); color: var(--hi); }
  .opt input { accent-color: var(--accent); margin: 0; }
  .sample { color: var(--dim); }
  .note { color: var(--faint); margin: 6px 0 0; }
  .range { width: 100%; accent-color: var(--accent); }
  .ticks { display: flex; justify-content: space-between; color: var(--faint); }
</style>
