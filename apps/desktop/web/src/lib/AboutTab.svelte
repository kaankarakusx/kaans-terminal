<!--
  ABOUT (OZELLIKLER.md §8, §10): sürüm, güncelleme (kontrol et / yeniden başlat), lisanslar, gizlilik, yerel hata
  günlüğü. Güncellemeler 10 dk boşta kalınca kendiliğinden kurulur; terminaller yaşar.
-->
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { openUrl } from '@tauri-apps/plugin-opener'
  import { inTauri } from './engine.svelte'
  import { duration } from './format'
  import { now } from './now.svelte'
  import { updates } from './updates.svelte'

  const PRIVACY = 'https://github.com/kaankarakusx/kaans-terminal/blob/main/PRIVACY.md'
  const v = $derived(updates.view)
  let err = $state<string | null>(null)

  const statusText = $derived.by(() => {
    switch (v.status) {
      case 'disabled':
        return inTauri ? 'updates are off in development builds' : 'browser preview'
      case 'checking':
        return 'checking…'
      case 'downloading':
        return `downloading ${v.version} · ${v.pct ?? 0}%`
      case 'ready':
        return `${v.version} is ready — installs when you're away 10 min, or restart now`
      case 'installing':
        return `installing ${v.version}…`
      case 'error':
        return `update failed: ${v.error ?? 'unknown error'}`
      default:
        return v.checked ? `up to date · checked ${duration((now.value - v.checked) / 1000)} ago` : 'up to date'
    }
  })

  async function call(cmd: string) {
    err = null
    if (!inTauri) return
    try {
      await invoke(cmd)
    } catch (e) {
      err = String(e)
    }
  }
</script>

<div class="about">
  <div class="name">Kaan's Terminal <span class="ver">v{v.current}</span></div>

  <h3>updates</h3>
  <div class="row">
    <span class="st" class:bad={v.status === 'error'} class:good={v.status === 'ready'}>{statusText}</span>
    {#if v.status === 'downloading'}
      <span class="bar" aria-hidden="true"><i style:width="{v.pct ?? 0}%"></i></span>
    {/if}
  </div>
  <div class="row btns">
    <button disabled={!inTauri || v.status === 'disabled' || v.status === 'checking' || v.status === 'downloading' || v.status === 'installing'} onclick={() => updates.check()}>check for updates</button>
    {#if v.status === 'ready'}
      <button class="primary" onclick={() => updates.install()}>↻ restart to update</button>
    {/if}
  </div>
  <p class="note">Terminals and AI chats keep running during an update; windows come back after the restart.</p>

  <h3>legal</h3>
  <div class="row btns">
    <button disabled={!inTauri} onclick={() => call('open_licenses')}>third-party licenses</button>
    <button disabled={!inTauri} onclick={() => void openUrl(PRIVACY)}>privacy</button>
  </div>
  <p class="note">MIT licensed. Your sessions, code and prompts never leave this computer.</p>

  <h3>troubleshooting</h3>
  <div class="row btns">
    <button disabled={!inTauri} onclick={() => call('reveal_error_log')}>show error log</button>
  </div>
  <p class="note">Crashes and freezes are written to a local log (secrets masked). Nothing is sent anywhere — attach it to a bug report if you like.</p>
  {#if err}<p class="err">✕ {err}</p>{/if}
</div>

<style>
  .about { display: grid; gap: 6px; }
  .name { color: var(--hi); font-weight: 700; font-size: 1.1em; }
  .ver { color: var(--dim); font-weight: 400; margin-left: 6px; }
  h3 { margin: 10px 0 0; color: var(--txt3); font-size: 0.85em; letter-spacing: 0.08em; text-transform: uppercase; font-weight: 600; }
  .row { display: flex; align-items: center; gap: 8px; min-width: 0; }
  .st { color: var(--txt2); }
  .st.bad { color: var(--red); }
  .st.good { color: var(--green); }
  .bar { flex: 1; max-width: 160px; height: 4px; background: var(--line); }
  .bar i { display: block; height: 100%; background: var(--accent); transition: width 0.3s; }
  .btns button { all: unset; padding: 2px 10px; border: 1px solid var(--line); color: var(--txt2); cursor: pointer; white-space: nowrap; }
  .btns button:hover:not([disabled]), .btns button:focus-visible { border-color: var(--accent); color: var(--hi); }
  .btns button[disabled] { opacity: 0.45; cursor: default; }
  .btns .primary { background: var(--accent); color: var(--ink); border-color: var(--accent); }
  .note { margin: 0; color: var(--dim); font-size: 0.9em; }
  .err { margin: 0; color: var(--red); }
</style>
