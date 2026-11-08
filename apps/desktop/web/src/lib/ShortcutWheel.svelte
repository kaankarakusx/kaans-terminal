<!-- Kısayol çarkı (OZELLIKLER.md §3.5): fare noktasında açılan radyal menü. -->
<script lang="ts">
  import { appActions } from './actions'
  import { ui } from './ui.svelte'

  const R = 92
  const items = $derived(ui.wheel ? appActions() : [])
  // Ekran kenarına taşmasın.
  const at = $derived(
    ui.wheel
      ? {
          x: Math.min(Math.max(ui.wheel.x, R + 60), window.innerWidth - R - 60),
          y: Math.min(Math.max(ui.wheel.y, R + 30), window.innerHeight - R - 30),
        }
      : { x: 0, y: 0 },
  )
  $effect(() => {
    if (ui.wheel) queueMicrotask(() => document.querySelector<HTMLButtonElement>('.wheel button')?.focus())
  })
</script>

{#if ui.wheel}
  <div
    class="veil"
    role="presentation"
    onpointerdown={(e) => e.target === e.currentTarget && (ui.wheel = null)}
    onkeydown={(e) => e.key === 'Escape' && (ui.wheel = null)}
  >
    <div class="wheel" role="menu" aria-label="shortcut wheel" style:left="{at.x}px" style:top="{at.y}px">
      {#each items as a, i (a.id)}
        {@const ang = (i / items.length) * Math.PI * 2 - Math.PI / 2}
        <button
          role="menuitem"
          style:--x="{Math.cos(ang) * R}px"
          style:--y="{Math.sin(ang) * R}px"
          style:animation-delay="{i * 20}ms"
          onclick={() => {
            ui.wheel = null
            a.run()
          }}
        >
          <span class="i">{a.icon}</span>
          <span class="l">{a.label}</span>
        </button>
      {/each}
      <button class="hub" aria-label="close" onclick={() => (ui.wheel = null)}>×</button>
    </div>
  </div>
{/if}

<style>
  .veil { position: fixed; inset: 0; z-index: 115; }
  .wheel { position: absolute; width: 0; height: 0; }
  button {
    all: unset;
    position: absolute;
    left: 0;
    top: 0;
    width: 74px;
    height: 54px;
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 2px;
    transform: translate(calc(var(--x) - 50%), calc(var(--y) - 50%));
    background: var(--panel);
    border: 1px solid var(--line);
    color: var(--txt2);
    cursor: pointer;
    backdrop-filter: blur(12px);
    animation: pop 0.18s ease-out backwards;
  }
  button:hover, button:focus-visible {
    color: var(--hi);
    border-color: var(--accent);
    box-shadow: 0 0 14px rgba(var(--accent-rgb), 0.45);
  }
  .i { color: var(--accent); font-size: 1.3em; }
  .l { font-size: 0.85em; white-space: nowrap; }
  .hub { --x: 0px; --y: 0px; width: 30px; height: 30px; border-radius: 50%; }
  @keyframes pop {
    from { opacity: 0; transform: translate(-50%, -50%) scale(0.6); }
  }
</style>
