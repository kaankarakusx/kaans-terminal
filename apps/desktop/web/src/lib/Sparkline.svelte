<!-- Etkinlik çizgisi: her sütun bir dakika; yükseklik o dakikadaki araç çağrısı. -->
<script lang="ts">
  let { values, label }: { values: number[]; label: string } = $props()

  const max = $derived(Math.max(1, ...values))
  const total = $derived(values.reduce((a, b) => a + b, 0))
</script>

<div class="spark" role="img" aria-label="{label}: {total}" title="{label}: {total}">
  {#each values as v, i (i)}
    <i class:on={v > 0} style:height="{v > 0 ? 15 + (85 * v) / max : 8}%"></i>
  {/each}
</div>

<style>
  .spark {
    height: 14px;
    display: flex;
    align-items: flex-end;
    gap: 1px;
  }
  i {
    flex: 1;
    background: var(--line-soft);
  }
  i.on {
    background: var(--hue, var(--accent));
    opacity: 0.85;
  }
</style>
