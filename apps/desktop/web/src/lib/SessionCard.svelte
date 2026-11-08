<!--
  Oturum kartı (TASARIM.md §5.3): üç ışık (kırmızı oturumu bitirir, sarı katlar, yeşil açar), proje, "window"
  rozeti, durum sayacı (WORKING mm:ss), başlık, klasör, şu an ne yaptığı, bağlam çubuğu, model · effort · sayılar,
  maliyet ve etkinlik çubukları.
-->
<script lang="ts">
  import Sparkline from './Sparkline.svelte'
  import { confirm } from './dialog.svelte'
  import { engine } from './engine.svelte'
  import { compact, prettyModel, stopwatch, tildePath, usd } from './format'
  import { focus } from './focus.svelte'
  import { now } from './now.svelte'
  import type { SessionView } from './types'
  import { killTerminal } from '../term/api'

  let {
    s,
    folded = false,
    winopen = false,
    onfold,
    onopen,
  }: {
    s: SessionView
    folded?: boolean
    /** Sohbetin penceresi açık: kart parlar */
    winopen?: boolean
    onfold?: () => void
    /** Sohbeti aç */
    onopen?: () => void
  } = $props()

  let el = $state<HTMLElement>()
  const focused = $derived(focus.id === s.id)
  $effect(() => {
    if (focused) el?.scrollIntoView({ block: 'nearest', behavior: 'smooth' })
  })

  const BADGE = { busy: 'WORKING', waiting: 'NEEDS YOU', idle: 'YOUR TURN', ended: 'ENDED' } as const
  const timer = $derived(
    s.status === 'busy' && s.turnStarted
      ? stopwatch(now.value - s.turnStarted)
      : s.status === 'waiting' && s.lastActivity
        ? stopwatch(now.value - s.lastActivity)
        : '',
  )
  const ctxLevel = $derived(
    s.contextPct == null ? 'none' : s.contextPct >= 85 ? 'high' : s.contextPct >= 60 ? 'mid' : 'low',
  )
  /** Son araç çağrısı (ne yaptığı) */
  const lastTool = $derived.by(() => {
    for (let i = engine.events.length - 1; i >= 0; i--) {
      const e = engine.events[i]
      if (e.session === s.id && e.kind === 'tool') return e
    }
    return undefined
  })
  const toolText = $derived(lastTool ? `${lastTool.tool ?? ''} ${(lastTool.target ?? '').split('/').filter(Boolean).at(-1) ?? ''}`.trim() : '')

  function end() {
    if (!s.terminal) return
    const id = s.terminal
    confirm.ask({
      title: `End "${s.title}"?`,
      body: 'The terminal running this chat is closed. Processes started from it are ended too.',
      action: 'end session',
      danger: true,
      run: () => killTerminal(id),
    })
  }
</script>

<article
  class="card {s.status}"
  class:folded
  class:focused
  class:winopen
  bind:this={el}
  style:--hue="var(--hue{s.hue})"
>
  <div class="row top">
    <span class="lights">
      <button
        class="light red"
        disabled={!s.terminal}
        title={s.terminal ? 'end session' : "only chats running in Kaan's Terminal can be ended here"}
        aria-label="end session"
        onclick={end}
      ></button>
      <button class="light amber" title={folded ? 'expand' : 'collapse to one line'} aria-label={folded ? 'expand card' : 'collapse card'} aria-expanded={!folded} onclick={onfold}></button>
      <button class="light green" title="open chat" aria-label="open chat" onclick={onopen}></button>
    </span>
    <button class="proj" title="open · {s.cwd}" onclick={onopen}>{s.project}</button>
    {#if s.tool !== 'claude'}<span class="tag">{s.tool}</span>{/if}
    {#if winopen}<span class="tag win">window</span>{/if}
    <span class="grow"></span>
    <span class="badge" title={s.status === 'busy' ? 'this turn has been running for' : undefined}>{BADGE[s.status]}{timer ? ` ${timer}` : ''}</span>
  </div>

  {#if !folded}
    <button class="title" title={s.title} onclick={onopen}>{s.title}</button>
    <div class="path" title={s.cwd}>{tildePath(s.cwd)}</div>

    <div class="now">
      {#if s.status === 'busy'}
        <span class="ring"></span><b>thinking…</b>
        {#if toolText}<span class="last">last <em>{toolText}</em></span>{/if}
      {:else if s.status === 'waiting'}
        <span class="ring"></span><b>needs you</b><span class="last">{s.waitingFor ?? ''}</span>
      {:else if s.status === 'idle'}
        <span class="ring"></span><b>your turn</b>{#if s.lastPrompt}<span class="last">› {s.lastPrompt}</span>{/if}
      {:else}
        <span class="ring"></span><b>ended</b>{#if s.lastPrompt}<span class="last">› {s.lastPrompt}</span>{/if}
      {/if}
    </div>

    <div
      class="ctx {ctxLevel}"
      title={s.contextWindow
        ? `context ${compact(s.contextTokens)} / ${compact(s.contextWindow)}`
        : `context ${compact(s.contextTokens)} (window unknown)`}
    >
      <span class="k">ctx</span>
      <div class="bar"><i style:width="{Math.min(100, s.contextPct ?? 0)}%"></i></div>
    </div>

    {#if s.sameFile.length}
      <div class="same" title="another chat wrote to the same file in the last 20 minutes:
{s.sameFile.join('\n')}">⚠ same file · {s.sameFile[0].split('/').at(-1)}{s.sameFile.length > 1 ? ` +${s.sameFile.length - 1}` : ''}</div>
    {/if}

    <div class="row stats">
      <span class="model">{prettyModel(s.model)}{s.effort ? ` · ${s.effort}` : ''}</span>
      <span title="tool calls today">tools <b>{s.toolCalls}</b></span>
      <span title="web calls today">web <b>{s.webCalls}</b></span>
      <span title="tool errors today">err <b class:err={s.toolErrors}>{s.toolErrors}</b></span>
      <span class="grow"></span>
      <span class="cost" title="today · api-eq">{s.tool === 'claude' ? `${usd(s.cost)}${s.unpriced ? '+' : ''}` : ''}</span>
      <span class="spark"><Sparkline values={s.activity.slice(-10)} label="tool calls per minute, last 10 minutes" /></span>
    </div>

    {#if s.subagents.length}
      <div class="subs">
        {#each s.subagents as a (a.id)}
          <span class="sub" title={a.description}>⬡ {a.kind}{a.lastTool ? ` · ${a.lastTool}` : ''}</span>
        {/each}
      </div>
    {/if}
  {/if}
</article>

<style>
  .card {
    position: relative;
    display: grid;
    /* Uzun satırlar (son araç, prompt) kartı genişletmesin, kısalsın */
    grid-template-columns: minmax(0, 1fr);
    gap: 4px;
    padding: 7px 10px 8px;
    border: 1px solid var(--line);
    background: rgba(var(--term-bg-rgb), 0.55);
    transition: border-color 0.3s, box-shadow 0.3s;
  }
  /* Çalışırken kenar parlar; seni beklerken uyarı renginde */
  .card.busy { border-color: rgba(var(--accent-rgb), 0.8); box-shadow: inset 0 0 18px rgba(var(--accent-rgb), 0.08); }
  .card.waiting { border-color: rgba(var(--warn-rgb), 0.8); box-shadow: inset 0 0 18px rgba(var(--warn-rgb), 0.08); }
  .card.idle { border-color: rgba(var(--cyan-rgb), 0.45); }
  .card.ended { opacity: 0.55; }
  .winopen { box-shadow: 0 0 0 1px rgba(var(--accent-rgb), 0.5), 0 0 16px rgba(var(--accent-rgb), 0.25); }
  .folded { padding-block: 4px; }
  .focused { animation: flash 1.1s ease-out 2; }
  @keyframes flash {
    0% { box-shadow: inset 0 0 0 1px var(--accent), 0 0 18px rgba(var(--accent-rgb), 0.55); }
    100% { box-shadow: inset 0 0 0 1px transparent, 0 0 0 transparent; }
  }

  .row { display: flex; align-items: center; gap: 8px; min-width: 0; white-space: nowrap; }
  .grow { flex: 1; }
  .lights { display: flex; gap: 5px; flex: none; }
  .light { all: unset; width: 10px; height: 10px; border-radius: 50%; cursor: pointer; opacity: 0.8; }
  .light:hover:not([disabled]), .light:focus-visible { opacity: 1; }
  .light:focus-visible { outline: 1px solid var(--hi); outline-offset: 2px; }
  .light[disabled] { opacity: 0.25; cursor: default; }
  .red { background: var(--red); }
  .amber { background: var(--amber); }
  .green { background: var(--green); }
  .red:hover:not([disabled]) { box-shadow: 0 0 6px var(--red); }
  .amber:hover { box-shadow: 0 0 6px var(--amber); }
  .green:hover { box-shadow: 0 0 6px var(--green); }

  .proj { all: unset; cursor: pointer; color: var(--hi); font-weight: 700; overflow: hidden; text-overflow: ellipsis; min-width: 0; }
  .proj:hover { color: var(--accent); }
  .proj:focus-visible { outline: 1px solid var(--accent); }
  .tag { flex: none; padding: 0 5px; border: 1px solid var(--line); color: var(--txt3); font-size: 0.85em; }
  .tag.win { color: var(--accent); border-color: rgba(var(--accent-rgb), 0.6); }
  .badge {
    flex: none;
    padding: 0 6px;
    border: 1px solid var(--line);
    color: var(--dim);
    font-weight: 700;
    font-size: 0.88em;
    letter-spacing: 0.05em;
    font-variant-numeric: tabular-nums;
  }
  .busy .badge { color: var(--accent); border-color: var(--accent); box-shadow: 0 0 8px rgba(var(--accent-rgb), 0.45); text-shadow: 0 0 6px rgba(var(--accent-rgb), 0.7); }
  .waiting .badge { color: var(--warn); border-color: var(--warn); box-shadow: 0 0 8px rgba(var(--warn-rgb), 0.45); animation: pulse 1.6s ease-in-out infinite; }
  .idle .badge { color: var(--cyan); border-color: rgba(var(--cyan-rgb), 0.55); }
  @keyframes pulse { 50% { opacity: 0.6; } }

  .title {
    all: unset;
    cursor: pointer;
    color: var(--hi);
    font-size: 1.08em;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .title:hover { text-decoration: underline; text-decoration-color: var(--accent); }
  .title:focus-visible { outline: 1px solid var(--accent); }
  .path { color: var(--dim); font-size: 0.9em; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

  .now { display: flex; align-items: center; gap: 6px; min-width: 0; white-space: nowrap; font-size: 0.95em; }
  .ring { flex: none; width: 7px; height: 7px; border-radius: 50%; border: 1.5px solid var(--dim); }
  .now b { font-weight: 600; color: var(--dim); }
  .busy .ring { border-color: var(--accent); }
  .busy .now b { color: var(--accent); }
  .waiting .ring { border-color: var(--warn); background: var(--warn); }
  .waiting .now b { color: var(--warn); }
  .idle .ring { border-color: var(--cyan); }
  .idle .now b { color: var(--cyan); }
  .last { color: var(--txt3); overflow: hidden; text-overflow: ellipsis; min-width: 0; }
  .last em { font-style: normal; color: var(--txt2); }

  .ctx { display: flex; align-items: center; gap: 8px; }
  .ctx .k { color: var(--dim); font-size: 0.9em; }
  .bar { flex: 1; height: 4px; background: var(--line-soft); }
  .bar i { display: block; height: 100%; background: var(--accent); box-shadow: 0 0 6px rgba(var(--accent-rgb), 0.5); transition: width 0.4s; }
  .ctx.mid .bar i { background: var(--amber); }
  .ctx.high .bar i { background: var(--red); box-shadow: 0 0 6px rgba(var(--red-rgb), 0.6); }

  .stats { color: var(--dim); font-size: 0.92em; gap: 10px; overflow: hidden; }
  .stats > span { flex: none; }
  .stats b { color: var(--txt); font-weight: 600; }
  .stats b.err { color: var(--red); }
  /* Dar kartta önce model adı kısalır; sayılar ve maliyet görünür kalır */
  .stats > .model { flex: 0 1 auto; min-width: 2ch; overflow: hidden; text-overflow: ellipsis; color: var(--txt3); }
  .cost { color: var(--amber); font-weight: 700; }
  .stats > .spark { width: 46px; }

  .same {
    color: var(--red);
    font-size: 0.85em;
    border: 1px solid rgba(var(--red-rgb), 0.5);
    padding: 0 4px;
    justify-self: start;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .subs { display: flex; flex-wrap: wrap; gap: 4px; }
  .sub { color: var(--violet); border: 1px solid rgba(var(--violet-rgb), 0.4); padding: 0 4px; font-size: 0.85em; }
  @media (prefers-reduced-motion: reduce) {
    .waiting .badge { animation: none; }
  }
</style>
