<!--
  Ayarlar sayfası (ana alanı kaplar; activity bar ⚙ ya da ⌘,): INTERFACE, INTEGRATIONS, SHORTCUTS, ABOUT.
  SHORTCUTS (OZELLIKLER.md §3.6): bir kısayola tıkla, yeni tuşlara bas. ↺ varsayılana döndürür.
  Kısayol çarkı için ikinci bir tetikleyici: fare tuşu.
-->
<script lang="ts">
  import AboutTab from './AboutTab.svelte'
  import { confirm } from './dialog.svelte'
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
      if (e.key === 'Escape' && !confirm.open && !e.defaultPrevented) ui.settings = false
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

<section class="page" aria-label="settings">
      <header>
        <span class="label">settings</span>
        <div class="tabs" role="tablist" aria-label="settings sections">
          {#each TABS as t (t)}
            <button role="tab" class="tab" class:on={ui.settingsTab === t} aria-selected={ui.settingsTab === t} onclick={() => (ui.settingsTab = t)}>{t}</button>
          {/each}
        </div>
        <span class="grow"></span>
        <button class="close" title="close (Esc)" aria-label="close settings" onclick={() => (ui.settings = false)}>×</button>
      </header>
      {#if warn}<div class="warn">⚠ {warn}</div>{/if}
      <div class="body"><div class="inner">
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
      </div></div>
</section>

<style>
  /* Tema/sıralama sayfasıyla aynı kabuk */
  .page {
    height: 100%;
    display: flex;
    flex-direction: column;
    min-height: 0;
    /* Opak: çok metinli sayfa, arkadaki paneller okunmayı bozmasın */
    background: rgb(var(--bg-rgb));
    border: 1px solid var(--line-soft);
    animation: fade 0.16s ease-out;
  }
  @keyframes fade { from { opacity: 0; } }
  @media (prefers-reduced-motion: reduce) { .page { animation: none; } }
  header { display: flex; align-items: center; gap: 10px; height: 22px; border-bottom: 1px solid var(--line-soft); }
  .label { height: 100%; display: flex; align-items: center; padding: 0 8px; background: var(--accent); color: var(--ink); font-weight: 700; letter-spacing: 0.04em; }
  .tabs { display: flex; height: 100%; }
  .tab { all: unset; height: 100%; display: flex; align-items: center; padding: 0 10px; color: var(--dim); font-weight: 700; cursor: pointer; border-bottom: 2px solid transparent; box-sizing: border-box; }
  .tab:hover { color: var(--hi); }
  .tab.on { color: var(--hi); border-bottom-color: var(--accent); }
  .tab:focus-visible { outline: 1px solid var(--hi); outline-offset: -1px; }
  .grow { flex: 1; }
  .close { all: unset; padding: 0 10px; color: var(--dim); cursor: pointer; }
  .close:hover, .close:focus-visible { color: var(--hi); }
  .warn { padding: 4px 10px; color: var(--amber); }
  .body { flex: 1; min-height: 0; overflow: auto; padding: 10px 16px 24px; }
  /* Okunur satır uzunluğu: geniş ekranda içerik ortada */
  .inner { max-width: 760px; margin: 0 auto; }
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
