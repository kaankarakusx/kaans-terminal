<!--
  Hacker name seçimi (OZELLIKLER.md §6.1, TASARIM.md §5.8): yazı makinesi efekti (hk-type), bekleme noktaları
  (hk-dots), hatada sallanma (hk-shake), başarıda yeşil parlama (hk-glow). Sonuç name#tag; tag sunucudan.
-->
<script lang="ts">
  import { NAME_RE, social } from './social.svelte'

  const TITLE = 'choose your hacker name'
  let typed = $state('')
  let name = $state(social.state.name ?? '')
  let busy = $state(false)
  let error = $state<string | null>(null)
  let shake = $state(false)
  let done = $state(false)
  let input = $state<HTMLInputElement>()

  // Yazı makinesi
  $effect(() => {
    let i = 0
    const t = setInterval(() => {
      typed = TITLE.slice(0, ++i)
      if (i >= TITLE.length) clearInterval(t)
    }, 38)
    input?.focus()
    return () => clearInterval(t)
  })

  const valid = $derived(NAME_RE.test(name))

  async function submit(e: SubmitEvent) {
    e.preventDefault()
    if (busy) return
    error = null
    busy = true
    try {
      await social.choose(name.trim())
      done = true
    } catch (err) {
      error = err instanceof Error ? err.message : String(err)
      shake = false
      requestAnimationFrame(() => (shake = true))
    } finally {
      busy = false
    }
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && !done) social.dismiss()
  }
</script>

<svelte:window {onkeydown} />

<div class="veil" role="presentation">
  <form class="box" class:shake class:glow={done} onsubmit={submit} aria-labelledby="hk-title">
    <h2 id="hk-title"><span class="prompt">❯</span> {typed}<span class="caret" aria-hidden="true"></span></h2>
    {#if done}
      <p class="ok">welcome, <b>{social.state.name}</b><span class="tag">#{social.state.tag}</span></p>
      <div class="actions"><button type="button" class="primary" onclick={() => (social.picking = false)}>enter</button></div>
    {:else}
      <p class="sub">Friends find you as <b>name#tag</b>. Letters, digits, <code>_ . -</code> · 3–20 characters.</p>
      <div class="field" class:bad={name.length > 0 && !valid}>
        <input
          bind:this={input}
          bind:value={name}
          maxlength="20"
          spellcheck="false"
          autocomplete="off"
          aria-label="hacker name"
          aria-invalid={name.length > 0 && !valid}
          placeholder="nightowl"
        />
        <span class="tag">#????</span>
      </div>
      {#if busy}<p class="wait">claiming<span class="dots"><i>.</i><i>.</i><i>.</i></span></p>{/if}
      {#if error}<p class="err" role="alert">✕ {error}</p>{/if}
      <div class="actions">
        <button type="button" onclick={() => social.dismiss()}>later</button>
        <button type="submit" class="primary" disabled={!valid || busy}>claim name</button>
      </div>
      <p class="note">Sent to the server: name#tag, app/OS version, active minutes, time zone, theme. Never your chats, code or files.</p>
    {/if}
  </form>
</div>

<style>
  .veil { position: fixed; inset: 0; z-index: 130; display: grid; place-content: center; background: rgba(0, 0, 0, 0.6); backdrop-filter: blur(4px); }
  .box { width: min(460px, 92vw); padding: 20px 22px; background: var(--panel); border: 1px solid var(--accent); box-shadow: 0 0 30px rgba(var(--accent-rgb), 0.25); }
  h2 { margin: 0 0 10px; color: var(--hi); font-size: 1.25em; font-weight: 700; min-height: 1.4em; }
  .prompt { color: var(--accent); }
  .caret { display: inline-block; width: 0.55em; height: 1em; margin-left: 2px; vertical-align: -0.12em; background: var(--accent); animation: blink 1s steps(1) infinite; }
  @keyframes blink { 50% { opacity: 0; } }
  .sub, .note { color: var(--dim); }
  .sub b, .ok b { color: var(--hi); }
  code { color: var(--txt2); }
  .field { display: flex; align-items: center; gap: 4px; margin: 12px 0 6px; padding: 6px 10px; border: 1px solid var(--line); background: rgba(var(--bg-rgb), 0.7); }
  .field:focus-within { border-color: var(--accent); }
  .field.bad { border-color: var(--red); }
  input { all: unset; flex: 1; color: var(--hi); font-size: 1.2em; font-weight: 700; }
  input::placeholder { color: var(--faint); }
  .tag { color: var(--dim); font-size: 1.1em; }
  .wait { color: var(--accent); margin: 4px 0; }
  .dots i { font-style: normal; animation: dot 1.2s infinite; }
  .dots i:nth-child(2) { animation-delay: 0.2s; }
  .dots i:nth-child(3) { animation-delay: 0.4s; }
  @keyframes dot { 0%, 100% { opacity: 0.2; } 40% { opacity: 1; } }
  .err { color: var(--red); margin: 4px 0; }
  .ok { font-size: 1.2em; color: var(--green); margin: 12px 0; }
  .actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 10px; }
  .actions button { all: unset; padding: 4px 14px; border: 1px solid var(--line); color: var(--txt2); cursor: pointer; }
  .actions button:hover:not([disabled]), .actions button:focus-visible { border-color: var(--accent); color: var(--hi); }
  .actions .primary { background: var(--accent); border-color: var(--accent); color: var(--ink); font-weight: 700; }
  .actions .primary:hover:not([disabled]) { color: var(--ink); background: var(--accent-soft); }
  .actions [disabled] { opacity: 0.4; cursor: default; }
  .note { margin: 14px 0 0; font-size: 0.85em; color: var(--faint); }
  .shake { animation: shake 0.4s; }
  @keyframes shake { 20%, 60% { transform: translateX(-7px); } 40%, 80% { transform: translateX(7px); } }
  .glow { border-color: var(--green); box-shadow: 0 0 34px rgba(var(--green-rgb), 0.45); transition: box-shadow 0.4s; }
  @media (prefers-reduced-motion: reduce) { .shake, .caret, .dots i { animation: none; } }
</style>
