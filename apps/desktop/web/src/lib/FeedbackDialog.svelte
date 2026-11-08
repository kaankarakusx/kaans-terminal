<!-- Geri bildirim (OZELLIKLER.md §6.5): doğrudan geliştiriciye, name#tag ve zamanla; e-posta istenmez. -->
<script lang="ts">
  import { news } from './news.svelte'
  import { social } from './social.svelte'
  import { ui } from './ui.svelte'

  let body = $state('')
  let busy = $state(false)
  let error = $state<string | null>(null)
  let sent = $state(false)

  async function submit(e: SubmitEvent) {
    e.preventDefault()
    busy = true
    error = await news.feedback(body)
    busy = false
    if (!error) sent = true
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') ui.feedback = false
  }
</script>

<svelte:window {onkeydown} />

<div class="veil" role="presentation" onclick={(e) => e.target === e.currentTarget && (ui.feedback = false)}>
  <form class="box" onsubmit={submit} aria-labelledby="fb-title">
    <h2 id="fb-title">feedback</h2>
    {#if !social.state.registered}
      <p class="sub">Pick a hacker name first, so the reply can find you.</p>
      <div class="actions">
        <button type="button" onclick={() => (ui.feedback = false)}>close</button>
        <button type="button" class="primary" onclick={() => { ui.feedback = false; social.picking = true }}>choose a hacker name</button>
      </div>
    {:else if sent}
      <p class="ok">✓ Thanks! It went straight to the developer.</p>
      <div class="actions"><button type="button" class="primary" onclick={() => (ui.feedback = false)}>close</button></div>
    {:else}
      <p class="sub">Bugs, ideas, anything. Sent as <b>{social.state.name}#{social.state.tag}</b> with the time and app version; no email needed.</p>
      <!-- svelte-ignore a11y_autofocus -->
      <textarea bind:value={body} rows="6" maxlength="4000" aria-label="your feedback" placeholder="The orbit map could…" autofocus></textarea>
      {#if error}<p class="err" role="alert">✕ {error}</p>{/if}
      <div class="actions">
        <span class="count">{body.length}/4000</span>
        <button type="button" onclick={() => (ui.feedback = false)}>cancel</button>
        <button type="submit" class="primary" disabled={busy || !body.trim()}>{busy ? 'sending…' : 'send'}</button>
      </div>
    {/if}
  </form>
</div>

<style>
  .veil { position: fixed; inset: 0; z-index: 125; display: grid; place-content: center; background: rgba(0, 0, 0, 0.55); }
  .box { width: min(480px, 92vw); padding: 16px 18px; background: rgb(var(--panel-rgb)); border: 1px solid var(--accent); box-shadow: 0 0 30px rgba(var(--accent-rgb), 0.2); }
  h2 { margin: 0 0 8px; color: var(--hi); font-size: 1.15em; }
  .sub { color: var(--dim); margin: 0 0 10px; }
  .sub b { color: var(--hi); }
  textarea { all: unset; box-sizing: border-box; display: block; width: 100%; padding: 8px; border: 1px solid var(--line); background: rgba(var(--bg-rgb), 0.6); color: var(--hi); white-space: pre-wrap; }
  textarea:focus { border-color: var(--accent); }
  .err { color: var(--red); }
  .ok { color: var(--green); }
  .actions { display: flex; align-items: center; justify-content: flex-end; gap: 8px; margin-top: 10px; }
  .count { margin-right: auto; color: var(--faint); }
  .actions button { all: unset; padding: 4px 14px; border: 1px solid var(--line); color: var(--txt2); cursor: pointer; }
  .actions button:hover:not([disabled]), .actions button:focus-visible { border-color: var(--accent); color: var(--hi); }
  .actions .primary { background: var(--accent); border-color: var(--accent); color: var(--ink); font-weight: 700; }
  .actions [disabled] { opacity: 0.4; cursor: default; }
</style>
