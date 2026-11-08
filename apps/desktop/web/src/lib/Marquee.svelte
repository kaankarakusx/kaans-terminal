<!-- Üst bardaki kayan yazı: son olaylar. Hareketi azalt açıksa kaymaz, en yeni olay sabit durur. -->
<script lang="ts">
  import { engine } from './engine.svelte'
  import type { EngineEvent } from './types'

  const MAX = 12

  function line(e: EngineEvent): string {
    const s = engine.sessionsById.get(e.session)
    const who = s?.title ?? e.session.slice(0, 8)
    switch (e.kind) {
      case 'prompt':
        return `${who} › ${e.target ?? ''}`
      case 'reply':
        return `${who} ‹ ${e.target ?? ''}`
      case 'toolError':
        return `${who} ✕ ${e.tool ?? 'tool'} failed`
      default:
        return `${who} · ${e.tool}${e.target ? ` ${e.target}` : ''}`
    }
  }

  const items = $derived(engine.events.slice(-MAX).toReversed().map(line))
  // Metin değişince animasyon baştan başlamasın diye süre içerik uzunluğuna bağlı.
  const duration = $derived(Math.max(30, items.join('').length / 9))
</script>

<div class="marquee" aria-live="off">
  {#if items.length}
    <div class="track" style:animation-duration="{duration}s">
      {#each [0, 1] as copy (copy)}
        <span class="run" aria-hidden={copy === 1}>
          {#each items as text, i (i)}<span class="item">{text}</span>{/each}
        </span>
      {/each}
    </div>
  {:else}
    <span class="idle">no activity yet</span>
  {/if}
</div>

<style>
  .marquee {
    flex: 1 1 0;
    min-width: 60px;
    overflow: hidden;
    color: var(--txt3);
    mask-image: linear-gradient(90deg, transparent, #000 4%, #000 96%, transparent);
  }
  .track {
    display: flex;
    width: max-content;
    animation: scroll linear infinite;
  }
  .run { display: flex; }
  .item { white-space: nowrap; padding-right: 2.5em; }
  .item::before { content: '▸ '; color: var(--accent); }
  .idle { color: var(--faint); }
  @keyframes scroll {
    to { transform: translateX(-50%); }
  }
  @media (prefers-reduced-motion: reduce) {
    .track { animation: none; }
    .run + .run { display: none; }
  }
</style>
