<!--
  + new terminal (OZELLIKLER.md §3.2, TASARIM.md §5.7): ortada klasör seçici; altında son açılan 5 klasör
  (⌘1…⌘5 ile hızlı açılır, ✕ listeden siler).
-->
<script lang="ts">
  import { inTauri } from './engine.svelte'
  import { forgetFolder, newTerminal, recentFolders } from './terminals'
  import { ui } from './ui.svelte'

  let path = $state('')
  let input = $state<HTMLInputElement>()
  let chosen = $state<string | null>(null)
  let error = $state<string | null>(null)

  const RECENT = 5
  let recent = $state<string[]>([])

  function forget(f: string) {
    forgetFolder(f)
    recent = recentFolders().slice(0, RECENT)
    input?.focus()
  }

  $effect(() => {
    if (ui.picker) {
      // Yerel değişkenle: efekt `recent`i hem yazıp hem okursa kendini tetikler (sonsuz döngü).
      const list = recentFolders().slice(0, RECENT)
      recent = list
      path = list[0] ?? '~'
      chosen = null
      error = null
      queueMicrotask(() => input?.select())
    }
  })

  const name = (p: string) => p.replace(/\/+$/, '').split('/').at(-1) || p
  const parent = (p: string) => p.replace(/\/+$/, '').split('/').slice(0, -1).join('/').replace(/^\/Users\/[^/]+/, '~')

  async function go(dir: string) {
    chosen = dir
    try {
      await newTerminal(dir.trim() || null)
      ui.picker = false
    } catch (e) {
      error = String(e)
      chosen = null
    }
  }

  async function browse() {
    if (!inTauri) return
    const { open } = await import('@tauri-apps/plugin-dialog')
    const dir = await open({ directory: true, multiple: false, defaultPath: path.startsWith('/') ? path : undefined })
    if (typeof dir === 'string') await go(dir)
  }
</script>

{#if ui.picker}
  <div class="veil" role="presentation" onclick={(e) => e.target === e.currentTarget && (ui.picker = false)}>
    <div class="ring" role="dialog" aria-modal="true" aria-label="new terminal">
      <form
        class="center"
        onsubmit={(e) => {
          e.preventDefault()
          void go(path)
        }}
      >
        <span class="tag">new terminal</span>
        <input
          bind:this={input}
          bind:value={path}
          aria-label="folder"
          spellcheck="false"
          onkeydown={(e) => {
            if (e.key === 'Escape') ui.picker = false
            // ⌘1…⌘5 (Windows'ta Ctrl): son açılan klasörler
            const n = Number(e.key)
            if ((e.metaKey || e.ctrlKey) && n >= 1 && n <= recent.length) {
              e.preventDefault()
              void go(recent[n - 1])
            }
          }}
        />
        {#if error}<span class="err">{error}</span>{/if}
        <div class="row">
          {#if inTauri}<button type="button" onclick={browse}>browse…</button>{/if}
          <button type="submit" class="go">open ↩</button>
        </div>
        {#if recent.length}
          <div class="recent" role="list" aria-label="recent folders">
            <span class="rlabel">recent</span>
            {#each recent as f, i (f)}
              <div class="rrow" role="listitem">
                <button type="button" class="ritem" class:chosen={chosen === f} title={f} onclick={() => go(f)}>
                  <b>{name(f)}</b>
                  <span class="rpath">{parent(f)}</span>
                  <kbd>{navigator.platform.startsWith('Mac') ? '⌘' : 'Ctrl+'}{i + 1}</kbd>
                </button>
                <button
                  type="button"
                  class="rx"
                  title="remove from recent"
                  aria-label="remove {name(f)} from recent"
                  onclick={() => forget(f)}>✕</button
                >
              </div>
            {/each}
          </div>
        {/if}
      </form>
    </div>
  </div>
{/if}

<style>
  .veil {
    position: fixed;
    inset: 0;
    z-index: 110;
    display: grid;
    place-content: center;
    background: rgba(0, 0, 0, 0.55);
    backdrop-filter: blur(4px);
  }
  .ring { position: relative; width: 0; height: 0; }
  .center {
    position: absolute;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    width: 300px;
    display: grid;
    gap: 6px;
    padding: 12px;
    background: var(--panel);
    border: 1px solid var(--accent);
    box-shadow: 0 0 24px rgba(var(--accent-rgb), 0.3);
  }
  .tag { justify-self: start; padding: 0 6px; background: var(--accent); color: var(--ink); font-weight: 700; }
  input {
    all: unset;
    padding: 4px 6px;
    color: var(--hi);
    background: rgba(var(--bg-rgb), 0.6);
    border: 1px solid var(--line);
  }
  input:focus { border-color: var(--accent); }
  .err { color: var(--red); font-size: 0.9em; }
  .row { display: flex; justify-content: flex-end; gap: 6px; }
  .row button { all: unset; padding: 2px 10px; border: 1px solid var(--line); color: var(--txt2); cursor: pointer; }
  .row button:hover, .row button:focus-visible { border-color: var(--accent); color: var(--hi); }
  .row .go { background: var(--accent); color: var(--ink); border-color: var(--accent); }
  .recent { display: grid; gap: 1px; margin-top: 4px; padding-top: 6px; border-top: 1px solid var(--line-soft); }
  .rlabel { color: var(--faint); font-size: 0.85em; letter-spacing: 0.08em; text-transform: uppercase; margin-bottom: 2px; }
  .rrow { display: flex; align-items: center; min-width: 0; }
  .rx {
    all: unset;
    flex: none;
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    margin-left: 2px;
    border-radius: 3px;
    font-size: 10px;
    color: var(--faint);
    cursor: pointer;
    opacity: 0.6;
  }
  .rrow:hover .rx, .rx:focus-visible { opacity: 1; }
  .rx:hover { color: var(--red); background: rgba(var(--red-rgb), 0.15); }
  .rx:focus-visible { outline: 1px solid var(--accent); }
  .ritem {
    all: unset;
    flex: 1;
    display: flex;
    align-items: baseline;
    gap: 8px;
    min-width: 0;
    padding: 3px 6px;
    cursor: pointer;
    border-left: 2px solid transparent;
  }
  .ritem b { color: var(--hi); font-weight: 600; flex: none; max-width: 45%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .rpath { flex: 1; min-width: 0; color: var(--dim); font-size: 0.85em; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .ritem kbd { flex: none; color: var(--faint); font-family: inherit; font-size: 0.85em; }
  .ritem:hover, .ritem:focus-visible { background: rgba(var(--accent-rgb), 0.12); border-left-color: var(--accent); }
  .ritem:hover kbd { color: var(--accent); }
  .ritem.chosen { background: rgba(var(--accent-rgb), 0.3); }

</style>
