<!--
  LEVEL UP / RANK UP (TASARIM.md §5.8). Level: altın rün halkası (0,9 sn) + kısa arp. Rank: büyük büyü çemberi
  (dönen iki halka + rün yazısı), yeni rozet, rütbe renginde parçacıklar, uzun fanfar. Yalnızca uygulama öndeyken
  gösterilir (sırada bekler); hareket azaltılmışsa yalnızca rozet ve metin solarak gelir.
-->
<script lang="ts">
  import RankBadge from './RankBadge.svelte'
  import { power } from './power.svelte'
  import { notifier } from './notify.svelte'
  import { fanfare, levelChime } from './sfx'
  import { xp, type Celebration } from './xp.svelte'

  let current = $state<Celebration | null>(null)

  $effect(() => {
    if (current || power.mode !== 'foreground' || xp.celebrations.length === 0) return
    const next = xp.celebrations.shift()!
    current = next
    if (notifier.sound) {
      if (next.kind === 'rank') fanfare()
      else levelChime()
    }
    const t = setTimeout(() => (current = null), next.kind === 'rank' ? 4200 : 2200)
    return () => clearTimeout(t)
  })

  const RUNES = 'ᚠᚢᚦᚨᚱᚲᚷᚹᚺᚾᛁᛃᛇᛈᛉᛊᛏᛒᛖᛗᛚᛜᛞᛟ'
  const ring = RUNES.split('').concat(RUNES.split('')).join(' ')
  const sparks = Array.from({ length: 28 }, (_, i) => ({ a: (i / 28) * 360, d: 0.6 + ((i * 37) % 40) / 100, delay: (i % 7) * 0.05 }))
</script>

{#if current}
  <div class="veil {current.kind}" style:--rc={current.rank.color} role="status" aria-live="polite">
    <div class="stage">
      <svg class="circle" viewBox="0 0 200 200" aria-hidden="true">
        <defs><path id="runepath" d="M100 100 m-78 0 a78 78 0 1 1 156 0 a78 78 0 1 1 -156 0" /></defs>
        <circle cx="100" cy="100" r="92" class="r1" />
        <circle cx="100" cy="100" r="64" class="r2" />
        <g class="spin"><text><textPath href="#runepath">{ring}</textPath></text></g>
        {#if current.kind === 'rank'}
          <g class="spin rev"><path d="M100 18 171 141H29z" class="tri" /><path d="M100 182 29 59h142z" class="tri" /></g>
        {/if}
      </svg>
      {#if current.kind === 'rank'}
        {#each sparks as p, i (i)}<i class="spark" style:--a="{p.a}deg" style:--d={p.d} style:animation-delay="{p.delay}s"></i>{/each}
      {/if}
      <div class="center">
        <RankBadge rank={current.rank} size={current.kind === 'rank' ? 96 : 56} glow />
      </div>
    </div>
    <div class="text">
      {#if current.kind === 'rank'}
        <small>RANK UP</small><b>{current.rank.name.toUpperCase()}</b><span>level {current.level}</span>
      {:else}
        <b>LEVEL UP · {current.level}</b>
      {/if}
    </div>
  </div>
{/if}

<style>
  .veil {
    position: fixed;
    inset: 0;
    z-index: 200;
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 14px;
    /* Kendiliğinden kaybolur; altındaki uygulamayı tıklamayı engellemez */
    pointer-events: none;
    background: radial-gradient(circle at center, color-mix(in oklch, var(--rc) 18%, transparent), transparent 55%);
    animation: fadeout 0.5s ease-in forwards;
    animation-delay: 1.7s;
  }
  .veil.rank { background: radial-gradient(circle at center, color-mix(in oklch, var(--rc) 26%, transparent), rgba(0, 0, 0, 0.55) 70%); animation-delay: 3.7s; }
  @keyframes fadeout { to { opacity: 0; } }
  .stage { position: relative; width: 260px; height: 260px; }
  .level .stage { width: 180px; height: 180px; }
  .circle { position: absolute; inset: 0; width: 100%; height: 100%; animation: open 0.9s cubic-bezier(0.2, 0.9, 0.3, 1.2) both; }
  @keyframes open { from { transform: scale(0.2) rotate(-40deg); opacity: 0; } }
  .r1, .r2, .tri { fill: none; stroke: var(--rc); }
  .r1 { stroke-width: 1.5; filter: drop-shadow(0 0 6px var(--rc)); }
  .r2 { stroke-width: 0.8; stroke-dasharray: 3 4; }
  .tri { stroke-width: 0.8; opacity: 0.7; }
  text { fill: var(--rc); font-size: 11px; letter-spacing: 2px; }
  .spin { transform-origin: 100px 100px; animation: spin 9s linear infinite; }
  .spin.rev { animation-direction: reverse; animation-duration: 14s; }
  @keyframes spin { to { transform: rotate(360deg); } }
  .center { position: absolute; inset: 0; display: grid; place-items: center; animation: pop 0.7s 0.25s cubic-bezier(0.2, 0.9, 0.3, 1.4) both; }
  @keyframes pop { from { transform: scale(0); opacity: 0; } }
  .spark {
    position: absolute;
    left: 50%;
    top: 50%;
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: var(--rc);
    box-shadow: 0 0 8px var(--rc);
    opacity: 0;
    animation: burst 1.6s ease-out forwards;
  }
  @keyframes burst {
    0% { transform: rotate(var(--a)) translateX(10px) scale(1); opacity: 1; }
    100% { transform: rotate(var(--a)) translateX(calc(170px * var(--d))) scale(0.2); opacity: 0; }
  }
  .text { display: grid; justify-items: center; gap: 2px; animation: rise 0.6s 0.35s ease-out both; text-shadow: 0 0 12px var(--rc); }
  @keyframes rise { from { transform: translateY(10px); opacity: 0; } }
  .text small { color: var(--hi); letter-spacing: 0.4em; font-weight: 700; }
  .text b { color: var(--rc); font-size: 2em; letter-spacing: 0.18em; font-family: var(--font-sig, inherit); }
  .level .text b { font-size: 1.5em; }
  .text span { color: var(--txt2); }
  @media (prefers-reduced-motion: reduce) {
    .circle, .spark { display: none; }
    .center, .text { animation: fadein 0.4s both; }
    @keyframes fadein { from { opacity: 0; } }
  }
</style>
