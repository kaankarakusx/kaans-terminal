<!--
  SHORTCUTS (OZELLIKLER.md §3.6): bir kısayola tıkla, yeni tuşlara bas. ↺ varsayılana döndürür.
  Kısayol çarkı için ikinci bir tetikleyici: fare tuşu.
-->
<script lang="ts">
  import AboutTab from './AboutTab.svelte'
  import IntegrationsTab from './IntegrationsTab.svelte'
  import InterfaceTab from './InterfaceTab.svelte'
  import type { SettingsTab } from './ui.svelte'
  import { comboOf, label, shortcuts, type WheelMouse } from './shortcuts.svelte'
  import { ui } from './ui.svelte'

  const TABS: SettingsTab[] = ['interface', 'integrations', 'shortcuts', 'about']

  let recording = $state<string | null>(null)
  let warn = $state<string | null>(null)

  const groups = $derived([
    { title: 'terminal windows', items: shortcuts.all.filter((s) => s.scope === 'term') },
    { title: 'dashboard', items: shortcuts.all.filter((s) => s.scope === 'app') },
  ])

  const MOUSE: { v: WheelMouse; l: string }[] = [
    { v: 'ctrl+middle', l: '⌃ + middle button' },
    { v: 'middle', l: 'middle button' },
    { v: 'back', l: 'back button' },
    { v: 'forward', l: 'forward button' },
    { v: 'none', l: 'none' },
  ]

  function onkeydown(e: KeyboardEvent) {
    if (!ui.settings) return
    if (!recording) {
      if (e.key === 'Escape') ui.settings = false
      return
    }
    e.preventDefault()
    e.stopPropagation()
    if (e.key === 'Escape') {
      recording = null
      return
    }
    const c = comboOf(e)
    if (!c) return
    const clash = shortcuts.conflict(recording, c)
    warn = clash ? `${label(c)} was also used by "${clash}"` : null
    shortcuts.set(recording, c)
    recording = null
  }
</script>

<svelte:window onkeydowncapture={onkeydown} />

{#if ui.settings}
  <div class="veil" role="presentation" onclick={(e) => e.target === e.currentTarget && (ui.settings = false)}>
    <div class="box" role="dialog" aria-modal="true" aria-labelledby="sc-title">
      <header role="tablist">
        <span class="sr" id="sc-title">settings</span>
        {#each TABS as t (t)}
          <button role="tab" class="tab" class:on={ui.settingsTab === t} aria-selected={ui.settingsTab === t} onclick={() => (ui.settingsTab = t)}>{t.toUpperCase()}</button>
        {/each}
        <button class="close" aria-label="close" onclick={() => (ui.settings = false)}>×</button>
      </header>
      {#if warn}<div class="warn">⚠ {warn}</div>{/if}
      <div class="body">
        {#if ui.settingsTab === 'integrations'}
          <IntegrationsTab />
        {:else if ui.settingsTab === 'interface'}
          <InterfaceTab />
        {:else if ui.settingsTab === 'about'}
          <AboutTab />
        {:else}
        {#each groups as g (g.title)}
          <h3>{g.title}</h3>
          {#each g.items as s (s.id)}
            <div class="row">
              <span class="name">{s.label}</span>
              <button
                class="key"
                class:rec={recording === s.id}
                onclick={() => {
                  recording = s.id
                  warn = null
                }}
              >
                {recording === s.id ? 'press keys…' : label(s.combo)}
              </button>
              <button class="reset" title="back to default" aria-label="reset {s.label}" disabled={s.isDefault} onclick={() => shortcuts.reset(s.id)}>↺</button>
            </div>
          {/each}
        {/each}
        <h3>shortcut wheel · second trigger</h3>
        <div class="mouse">
          {#each MOUSE as m (m.v)}
            <label>
              <input type="radio" name="wheel-mouse" checked={shortcuts.saved.wheelMouse === m.v} onchange={() => shortcuts.setWheelMouse(m.v)} />
              {m.l}
            </label>
          {/each}
        </div>
        <p class="note">Tab switching (⌘1–⌘9), scrollback (⌘↑ ⌘↓ ⌘⇞ ⌘⇟ ⌘↖ ⌘↘) and copy/paste are fixed.</p>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .veil { position: fixed; inset: 0; z-index: 118; display: grid; place-content: center; background: rgba(0, 0, 0, 0.5); }
  .box {
    width: min(680px, 92vw);
    max-height: 82vh;
    display: flex;
    flex-direction: column;
    background: var(--panel);
    border: 1px solid var(--line);
    backdrop-filter: blur(14px);
  }
  header { display: flex; align-items: center; height: 22px; border-bottom: 1px solid var(--line-soft); }
  .tab { all: unset; height: 100%; display: flex; align-items: center; padding: 0 10px; color: var(--dim); font-weight: 700; cursor: pointer; }
  .tab:hover { color: var(--hi); }
  .tab.on { background: var(--accent); color: var(--ink); }
  .tab:focus-visible { outline: 1px solid var(--hi); outline-offset: -1px; }
  .sr { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0 0 0 0); }
  .close { all: unset; margin-left: auto; padding: 0 8px; color: var(--dim); cursor: pointer; }
  .close:hover { color: var(--hi); }
  .warn { padding: 4px 10px; color: var(--amber); }
  .body { overflow: auto; padding: 6px 10px 12px; }
  h3 { margin: 10px 0 4px; color: var(--dim); font-size: 0.85em; font-weight: 600; text-transform: uppercase; letter-spacing: 0.05em; }
  .row { display: grid; grid-template-columns: 1fr auto 22px; gap: 8px; align-items: center; padding: 1px 0; }
  .name { color: var(--txt2); }
  .key {
    all: unset;
    min-width: 90px;
    text-align: center;
    padding: 1px 8px;
    border: 1px solid var(--line);
    color: var(--hi);
    cursor: pointer;
  }
  .key:hover, .key:focus-visible { border-color: var(--accent); }
  .key.rec { border-color: var(--accent); color: var(--accent); animation: blink 1s ease-in-out infinite; }
  @keyframes blink { 50% { opacity: 0.5; } }
  .reset { all: unset; color: var(--dim); cursor: pointer; text-align: center; }
  .reset:hover { color: var(--hi); }
  .reset[disabled] { opacity: 0.25; cursor: default; }
  .mouse { display: flex; flex-wrap: wrap; gap: 4px 14px; color: var(--txt2); }
  .mouse input { accent-color: var(--accent); }
  .note { color: var(--faint); margin: 10px 0 0; }
</style>
