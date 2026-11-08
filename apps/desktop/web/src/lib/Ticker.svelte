<!-- Alt şerit: son 200 araç çağrısı, kategori renginde kareler; hata kırmızı. -->
<script lang="ts">
  import { engine } from './engine.svelte'

  const tools = $derived(engine.events.filter((e) => e.kind === 'tool'))
</script>

<footer class="ticker">
  <span class="tag">tool.stream</span>
  <div class="squares">
    {#each tools as e, i (i + ':' + e.id)}
      <i
        style:background={e.id && engine.failed.has(e.id) ? 'var(--red)' : `var(--cat-${e.category ?? 'other'})`}
        title="{e.tool} {e.target ?? ''}"
      ></i>
    {/each}
  </div>
  <span class="meta">{tools.length}</span>
</footer>

<style>
  .ticker {
    height: 22px;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 8px;
    background: var(--panel);
    border: 1px solid var(--line);
    overflow: hidden;
  }
  .tag { color: var(--accent); font-weight: 700; flex: none; }
  .squares { flex: 1; display: flex; gap: 2px; overflow: hidden; justify-content: flex-end; }
  i { flex: none; width: 7px; height: 9px; opacity: 0.9; }
  .meta { color: var(--dim); flex: none; }
</style>
