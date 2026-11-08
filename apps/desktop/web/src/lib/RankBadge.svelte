<!--
  Rütbe rozeti (TASARIM.md §5.8): altıgen çerçeve + rütbenin amblemi. Çizimler bizim; renk temadan bağımsız
  (rütbenin kendi rengi, packages/shared/src/xp.ts RANKS).
-->
<script lang="ts">
  import type { Rank } from '@kaans/shared/xp'

  let { rank, size = 24, glow = false }: { rank: Rank; size?: number; glow?: boolean } = $props()
</script>

<svg
  class="badge"
  class:glow
  class:eternal={rank.id === 'eternal'}
  width={size}
  height={size}
  viewBox="0 0 32 32"
  style:--rc={rank.color}
  role="img"
  aria-label="{rank.name} rank"
>
  <path class="frame" d="M16 1.8 28.3 8.9v14.2L16 30.2 3.7 23.1V8.9z" />
  {#if rank.id === 'eternal'}<circle class="halo" cx="16" cy="16" r="12.2" />{/if}
  <g class="emblem">
    {#if rank.id === 'apprentice'}
      <!-- mum -->
      <rect x="13" y="15" width="6" height="9" rx="1" />
      <path d="M16 8.5c2 2.2 2.2 3.8 0 5.6-2.2-1.8-2-3.4 0-5.6z" class="fill" />
    {:else if rank.id === 'adept'}
      <!-- rün taşı -->
      <path d="M11 10.5 16 8l5 2.5 1 9-6 4.5-6-4.5z" />
      <path d="M16 11.5v9M16 14l3-2M16 17l-3-2" />
    {:else if rank.id === 'conjurer'}
      <!-- çağırma halkası -->
      <circle cx="16" cy="16" r="7.5" />
      <path d="M16 11.5l3.9 2.25v4.5L16 20.5l-3.9-2.25v-4.5z" class="fill" />
    {:else if rank.id === 'runesmith'}
      <!-- örs ve rün -->
      <path d="M9 15h12l-2.5 3h-7zM12.5 18v3h7v-3M10.5 21h11" />
      <path d="M16 7.5v5M14 9l2 1.5 2-1.5" />
    {:else if rank.id === 'archmage'}
      <!-- yıldızlı asa -->
      <path d="M13 24.5 18.5 13" />
      <path d="M19.5 7.5l1 2.3 2.4.3-1.8 1.6.5 2.4-2.1-1.2-2.1 1.2.5-2.4-1.8-1.6 2.4-.3z" class="fill" />
    {:else if rank.id === 'oracle'}
      <!-- üçüncü göz -->
      <path d="M8.5 16c4.5-5.5 10.5-5.5 15 0-4.5 5.5-10.5 5.5-15 0z" />
      <circle cx="16" cy="16" r="2.3" class="fill" />
      <path d="M16 7.5v2M10.5 9.5l1 1.5M21.5 9.5l-1 1.5" />
    {:else}
      <!-- taç -->
      <path d="M9.5 20.5 8.5 12l4 3.2L16 9.5l3.5 5.7 4-3.2-1 8.5z" class="fill" />
      <path d="M10 23h12" />
    {/if}
  </g>
</svg>

<style>
  .badge { flex: none; overflow: visible; }
  .frame { fill: color-mix(in oklch, var(--rc) 16%, var(--bg)); stroke: var(--rc); stroke-width: 1.6; }
  .emblem { fill: none; stroke: var(--rc); stroke-width: 1.6; stroke-linecap: round; stroke-linejoin: round; }
  .emblem .fill { fill: var(--rc); }
  .glow { filter: drop-shadow(0 0 4px var(--rc)); }
  .halo { fill: none; stroke: var(--hi); stroke-width: 0.8; stroke-dasharray: 2 3; opacity: 0.7; transform-origin: 16px 16px; animation: spin 12s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
  @media (prefers-reduced-motion: reduce) { .halo { animation: none; } }
</style>
