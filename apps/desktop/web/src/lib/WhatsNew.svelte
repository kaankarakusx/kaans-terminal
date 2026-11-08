<!-- Yeni sürümün ilk açılışında bir kez: sürüm notları (Rust `update_whats_new`). -->
<script lang="ts">
  import RichText from './RichText.svelte'
  import { updates } from './updates.svelte'
</script>

{#if updates.whatsNew}
  {@const w = updates.whatsNew}
  <div class="veil" role="presentation" onclick={(e) => e.target === e.currentTarget && (updates.whatsNew = null)}>
    <div class="box" role="dialog" aria-modal="true" aria-labelledby="wn-title">
      <span class="tag">what's new</span>
      <h2 id="wn-title">Kaan's Terminal {w.version}</h2>
      <div class="notes">
        {#each (w.notes || 'Bug fixes and improvements.').split('\n') as line, i (i)}
          {#if line.trim()}<p><RichText text={line.replace(/^[-*]\s+/, '· ')} /></p>{/if}
        {/each}
      </div>
      <button class="ok" onclick={() => (updates.whatsNew = null)}>got it</button>
    </div>
  </div>
{/if}

<style>
  .veil { position: fixed; inset: 0; z-index: 120; display: grid; place-content: center; background: rgba(0, 0, 0, 0.55); backdrop-filter: blur(4px); }
  .box { width: min(460px, calc(100vw - 32px)); max-height: 70vh; display: grid; grid-template-rows: auto auto 1fr auto; gap: 8px; padding: 14px; background: var(--panel); border: 1px solid var(--accent); box-shadow: 0 0 24px rgba(var(--accent-rgb), 0.3); }
  .tag { justify-self: start; padding: 0 6px; background: var(--accent); color: var(--ink); font-weight: 700; }
  h2 { margin: 0; color: var(--hi); font-size: 1.1em; }
  .notes { overflow: auto; color: var(--txt2); }
  .notes p { margin: 0 0 4px; }
  .ok { all: unset; justify-self: end; padding: 2px 12px; background: var(--accent); color: var(--ink); cursor: pointer; }
  .ok:focus-visible { outline: 1px solid var(--hi); outline-offset: 2px; }
</style>
