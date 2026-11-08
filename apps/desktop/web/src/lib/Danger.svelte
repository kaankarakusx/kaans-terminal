<!--
  DANGER (OZELLIKLER.md §2.5): ebeveyni ölmüş, terminali olmayan ve 2 dakikadır yarım çekirdekten fazla CPU yakan
  süreç. Alarm çalar (ses açıksa), panel ne olduğunu söyler, tek tıkla bitirilir (onaydan sonra).
-->
<script lang="ts">
  import { ago } from './format'
  import { confirm } from './dialog.svelte'
  import { engine } from './engine.svelte'
  import { notifier } from './notify.svelte'
  import { now } from './now.svelte'
  import { alarm } from './sfx'
  import * as api from '../term/api'

  const list = $derived(engine.snapshot?.runaways ?? [])
  let seen = new Set<number>()
  let hidden = $state<Set<number>>(new Set())
  const visible = $derived(list.filter((r) => !hidden.has(r.pid)))

  $effect(() => {
    const fresh = list.filter((r) => !seen.has(r.pid))
    if (fresh.length && notifier.sound) alarm()
    seen = new Set(list.map((r) => r.pid))
  })

  function end(r: (typeof list)[number]) {
    confirm.ask({
      title: `End ${r.name}?`,
      body: `pid ${r.pid} · ${r.cpuPct}% CPU · no terminal, parent gone${r.path ? ` · ${r.path}` : ''}`,
      action: 'end process',
      danger: true,
      run: () => api.dangerKill(r.pid),
    })
  }
</script>

{#if visible.length}
  <div class="danger" role="alert">
    <header><b>DANGER</b> runaway process{visible.length > 1 ? 'es' : ''}</header>
    {#each visible as r (r.pid)}
      <div class="row">
        <span class="name" title={r.path ?? r.name}>{r.name}</span>
        <span class="cpu">{r.cpuPct}%</span>
        <span class="since">{ago(now.value - r.since)}</span>
        <button class="end" onclick={() => end(r)}>end</button>
        <button class="x" aria-label="hide" title="hide" onclick={() => (hidden = new Set([...hidden, r.pid]))}>×</button>
      </div>
    {/each}
  </div>
{/if}

<style>
  .danger {
    position: fixed;
    top: 64px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 60;
    min-width: 340px;
    max-width: 90vw;
    background: var(--panel);
    border: 1px solid var(--warn);
    box-shadow: 0 0 24px rgba(var(--warn-rgb), 0.45);
    animation: dp-hit 0.45s ease-out;
  }
  @keyframes dp-hit {
    0% { transform: translateX(-50%) scale(1.06); }
    25% { transform: translateX(calc(-50% - 6px)); }
    50% { transform: translateX(calc(-50% + 5px)); }
    75% { transform: translateX(calc(-50% - 3px)); }
    100% { transform: translateX(-50%); }
  }
  header { padding: 2px 8px; color: var(--txt2); border-bottom: 1px solid rgba(var(--warn-rgb), 0.4); }
  header b { color: var(--ink); background: var(--warn); padding: 0 6px; margin-right: 6px; }
  .row { display: grid; grid-template-columns: 1fr auto auto auto auto; gap: 8px; align-items: center; padding: 3px 8px; }
  .name { color: var(--hi); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .cpu { color: var(--warn); font-weight: 700; }
  .since { color: var(--dim); }
  button { all: unset; cursor: pointer; padding: 0 6px; }
  .end { color: var(--ink); background: var(--warn); }
  .end:focus-visible { outline: 1px solid var(--hi); }
  .x { color: var(--dim); }
  .x:hover { color: var(--hi); }
</style>
