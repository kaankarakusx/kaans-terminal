<script lang="ts">
  import { menu } from './menu.svelte'

  let el = $state<HTMLDivElement>()

  // Ekran dışına taşmasın.
  const pos = $derived.by(() => {
    const m = menu.open
    if (!m) return { x: 0, y: 0 }
    const w = el?.offsetWidth ?? 180
    const h = el?.offsetHeight ?? 120
    return {
      x: Math.min(m.x, window.innerWidth - w - 6),
      y: Math.min(m.y, window.innerHeight - h - 6),
    }
  })

  $effect(() => {
    if (!menu.open) return
    el?.querySelector<HTMLButtonElement>('button:not([disabled])')?.focus()
    const onDown = (e: PointerEvent) => {
      if (el && !el.contains(e.target as Node)) menu.close()
    }
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') menu.close()
    }
    window.addEventListener('pointerdown', onDown, true)
    window.addEventListener('keydown', onKey)
    window.addEventListener('blur', menu.close)
    return () => {
      window.removeEventListener('pointerdown', onDown, true)
      window.removeEventListener('keydown', onKey)
      window.removeEventListener('blur', menu.close)
    }
  })
</script>

{#if menu.open}
  <div class="menu" role="menu" bind:this={el} style:left="{pos.x}px" style:top="{pos.y}px">
    {#each menu.open.items as item, i (i)}
      {#if item === 'sep'}
        <hr />
      {:else}
        <button
          role="menuitem"
          disabled={item.disabled}
          class:danger={item.danger}
          onclick={() => {
            menu.close()
            item.action()
          }}
        >
          <span class="check">{item.checked === undefined ? '' : item.checked ? '✓' : ''}</span>
          {item.label}
        </button>
      {/if}
    {/each}
  </div>
{/if}

<style>
  .menu {
    position: fixed;
    z-index: 100;
    min-width: 170px;
    padding: 4px;
    display: grid;
    background: var(--panel);
    border: 1px solid var(--line);
    backdrop-filter: blur(14px);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
    animation: pl-in 0.12s ease-out;
  }
  @keyframes pl-in {
    from { opacity: 0; transform: translateY(-4px); }
  }
  button {
    all: unset;
    display: flex;
    gap: 6px;
    padding: 3px 8px;
    color: var(--txt2);
    cursor: pointer;
    white-space: nowrap;
  }
  button:hover, button:focus-visible { background: rgba(var(--accent-rgb), 0.16); color: var(--hi); }
  button[disabled] { color: var(--faint); cursor: default; background: none; }
  .danger { color: var(--red); }
  .check { width: 1em; color: var(--accent); }
  hr { border: 0; border-top: 1px solid var(--line-soft); margin: 3px 0; }
</style>
