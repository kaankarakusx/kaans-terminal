<!-- Köşe bildirimleri: durum sürdükçe kalır; × ile kapatılır (durum bitip tekrar olursa yeniden gelir). -->
<script lang="ts">
  import { fly } from 'svelte/transition'
  import { ago } from './format'
  import { engine } from './engine.svelte'
  import { focus } from './focus.svelte'
  import { openChat } from './terminals'
  import { notifier } from './notify.svelte'
  import { now } from './now.svelte'

  const reduced =
    typeof matchMedia !== 'undefined' && matchMedia('(prefers-reduced-motion: reduce)').matches
</script>

<div class="toasts" aria-live="polite">
  {#each notifier.toasts.slice(0, 5) as t (t.key)}
    <div class="toast {t.kind}" transition:fly={{ x: 40, duration: reduced ? 0 : 220 }}>
      <button
        class="body"
        onclick={() => {
          const s = t.session ? engine.sessionsById.get(t.session) : undefined
          if (t.kind === 'waiting' && s) void openChat(s)
          else if (t.session) focus.session(t.session)
        }}
      >
        <span class="kind">{t.kind === 'waiting' ? '● needs you' : '⚠ same file'}</span>
        <span class="title">{t.title}</span>
        <span class="detail">{t.detail}{t.since ? ` · ${ago(now.value - t.since)}` : ''}</span>
      </button>
      <button class="x" aria-label="dismiss" title="dismiss" onclick={() => notifier.dismiss(t.key)}>×</button>
    </div>
  {/each}
  {#if notifier.toasts.length > 5}
    <div class="more">+{notifier.toasts.length - 5} more</div>
  {/if}
</div>

<style>
  /* Konum App.svelte'deki .corner sütunundan (mesaj bildirimleriyle aynı yığın) */
  .toasts {
    display: grid;
    gap: 6px;
    width: min(320px, calc(100vw - 32px));
    pointer-events: none;
  }
  .toast {
    pointer-events: auto;
    display: flex;
    align-items: stretch;
    background: var(--panel);
    border: 1px solid var(--line);
    border-left: 3px solid var(--warn);
    backdrop-filter: blur(14px);
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.45), 0 0 14px rgba(var(--warn-rgb), 0.25);
  }
  .toast.clash { border-left-color: var(--red); box-shadow: 0 6px 20px rgba(0, 0, 0, 0.45), 0 0 14px rgba(var(--red-rgb), 0.25); }
  .body {
    all: unset;
    flex: 1;
    min-width: 0;
    display: grid;
    gap: 1px;
    padding: 6px 8px;
    cursor: pointer;
  }
  .body:focus-visible { outline: 1px solid var(--accent); }
  .kind { color: var(--warn); font-weight: 700; font-size: 0.85em; }
  .clash .kind { color: var(--red); }
  .title { color: var(--hi); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .detail { color: var(--dim); font-size: 0.9em; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .x {
    all: unset;
    padding: 0 8px;
    color: var(--dim);
    cursor: pointer;
  }
  .x:hover, .x:focus-visible { color: var(--hi); }
  .more { color: var(--dim); text-align: right; font-size: 0.85em; }
</style>
