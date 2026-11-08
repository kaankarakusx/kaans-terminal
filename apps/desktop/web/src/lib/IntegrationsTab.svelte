<!--
  INTEGRATIONS (MIMARI.md §7): harici araç ayarlarına dokunan her şey burada, ayrı anahtarla ve ne yaptığıyla.
  Hiçbiri kendiliğinden açılmaz; her değişiklikten önce yedek alınır; "remove all" her şeyi geri alır.
-->
<script lang="ts">
  import { onMount } from 'svelte'
  import { confirm } from './dialog.svelte'
  import * as api from './integrations'

  interface Row {
    id: string
    enabled: boolean
    error: string | null
    notes: string[]
  }

  let rows = $state<Row[]>([])
  let busy = $state<string | null>(null)
  let error = $state<string | null>(null)
  let loading = $state(true)

  const INFO: Record<string, { title: string; what: string; files: string; group: 'limits' | 'look' | 'savers' }> = {
    'claude-statusline': {
      group: 'limits',
      title: 'Claude limits in the top bar',
      what: "Sets Claude Code's statusLine to this app. It saves only the 5-hour and weekly usage (rate_limits) to a local file — nothing leaves your computer. Your own statusLine keeps running and its output is shown as before.",
      files: '~/.claude/settings.json → statusLine',
    },
    'claude-theme': {
      group: 'look',
      title: 'Paint Claude Code with the app theme',
      what: "Switches Claude Code to its dark-ansi theme, which uses your terminal's 16 colors — so changing the theme here recolors Claude too. Turning it off restores your previous Claude theme. (Codex and Antigravity: not yet — their theme settings aren't documented.)",
      files: '~/.claude/settings.json → theme',
    },
    rtk: {
      group: 'savers',
      title: 'rtk — shorter command output',
      what: 'Rewrites shell commands so their output is compressed before the AI reads it. Uses your own rtk if installed; otherwise the bundled rtk-oss is placed in ~/.local/bin. Installed and removed with rtk\'s own init/--uninstall; telemetry stays off.',
      files: '~/.claude/settings.json hooks, ~/.claude/CLAUDE.md (@RTK.md), Codex hooks.json + AGENTS.md, Antigravity plugin',
    },
    caveman: {
      group: 'savers',
      title: 'caveman — terse answers',
      what: "Installs the caveman plugin with Claude Code's own plugin commands (and Gemini's extension command if Gemini is installed). Needs the network once. Turning it off uninstalls it; a marketplace you added yourself is left alone.",
      files: 'Claude plugins (caveman@caveman)',
    },
  }
  const GROUPS = [
    { key: 'limits', title: 'limits' },
    { key: 'look', title: 'look' },
    { key: 'savers', title: 'token savers · off by default' },
  ] as const

  async function refresh() {
    try {
      const [ints, savers] = await Promise.all([api.status(), api.savers()])
      rows = [
        ...ints.map((i) => ({
          id: i.id,
          enabled: i.enabled,
          error: i.error,
          notes: [
            ...(i.chained && i.id === 'claude-statusline' ? [`chained: ${i.chained}`] : []),
            ...(i.chained && i.id === 'claude-theme' && !i.enabled ? [`current Claude theme: ${i.chained}`] : []),
            ...(i.devBuild && !i.enabled ? ['development build: the setting would point at it'] : []),
          ],
        })),
        ...savers.map((s) => ({
          id: s.id,
          enabled: s.enabled,
          error: s.error,
          notes: [
            ...(s.detail ? [s.detail] : []),
            ...(s.id === 'rtk' ? [s.source === 'yours' ? 'using your own rtk' : s.source === 'bundled' ? 'using the bundled copy' : 'bundled copy will be installed when you turn it on'] : []),
            ...(s.agents.length ? [`active for: ${s.agents.join(', ')}`] : []),
          ],
        })),
      ]
      error = null
    } catch (e) {
      error = String(e)
    } finally {
      loading = false
    }
  }

  async function toggle(r: Row) {
    busy = r.id
    try {
      await api.setEnabled(r.id, !r.enabled)
    } catch (e) {
      error = `${INFO[r.id]?.title ?? r.id}: ${e}`
    } finally {
      busy = null
      await refresh()
    }
  }

  function removeAll() {
    confirm.ask({
      title: 'Remove all integrations?',
      body: 'Every setting this app changed in other tools goes back to how it was: statusLine and Claude theme restored, rtk hooks and the caveman plugin removed.',
      action: 'remove all',
      danger: true,
      run: async () => {
        busy = '*'
        try {
          await api.removeAll()
        } catch (e) {
          error = String(e)
        } finally {
          busy = null
          await refresh()
        }
      },
    })
  }

  onMount(refresh)
</script>

<p class="lead">
  These change settings of other tools on your computer. Nothing is switched on without you, every file is backed up
  before a change, and everything can be undone.
</p>
{#if error}<div class="err">{error}</div>{/if}
{#if loading}<p class="dim">checking…</p>{/if}
{#each GROUPS as g (g.key)}
  {@const list = rows.filter((r) => INFO[r.id]?.group === g.key)}
  {#if list.length}
    <h3>{g.title}</h3>
    {#each list as r (r.id)}
      {@const info = INFO[r.id]}
      <div class="item" class:on={r.enabled}>
        <div class="head">
          <b>{info?.title ?? r.id}</b>
          {#if busy === r.id}<span class="dim">working…</span>{/if}
          <button
            class="switch"
            role="switch"
            aria-checked={r.enabled}
            aria-label={info?.title ?? r.id}
            disabled={!!busy || (!!r.error && !r.enabled)}
            onclick={() => toggle(r)}
          >
            <i></i>
          </button>
        </div>
        <p>{info?.what}</p>
        <p class="files">changes: {info?.files} (backed up first)</p>
        {#each r.notes as n (n)}<p class="files">{n}</p>{/each}
        {#if r.error}<p class="err">{r.error}</p>{/if}
      </div>
    {/each}
  {/if}
{/each}
<button class="remove" disabled={!!busy} onclick={removeAll}>remove all integrations</button>

<style>
  .lead { color: var(--txt3); margin: 4px 0 10px; }
  .dim { color: var(--dim); }
  .item { border: 1px solid var(--line-soft); padding: 8px 10px; margin-bottom: 8px; }
  .item.on { border-color: rgba(var(--accent-rgb), 0.55); }
  .head { display: flex; align-items: center; gap: 10px; }
  .head b { color: var(--hi); flex: 1; }
  p { margin: 4px 0 0; color: var(--txt2); }
  .files { color: var(--dim); font-size: 0.9em; }
  .err { color: var(--red); }
  h3 { margin: 12px 0 4px; color: var(--dim); font-size: 0.85em; font-weight: 600; text-transform: uppercase; letter-spacing: 0.05em; }
  .switch {
    all: unset;
    position: relative;
    flex: none;
    width: 30px;
    height: 16px;
    border: 1px solid var(--line);
    background: rgba(var(--bg-rgb), 0.6);
    cursor: pointer;
  }
  .switch i { position: absolute; top: 2px; left: 2px; width: 10px; height: 10px; background: var(--dim); transition: left 0.15s, background 0.15s; }
  .switch[aria-checked='true'] { border-color: var(--accent); }
  .switch[aria-checked='true'] i { left: 16px; background: var(--accent); box-shadow: 0 0 6px var(--accent); }
  .switch:focus-visible { outline: 1px solid var(--hi); }
  .switch[disabled] { opacity: 0.4; cursor: default; }
  .remove { all: unset; margin-top: 12px; padding: 2px 10px; border: 1px solid rgba(var(--warn-rgb), 0.6); color: var(--warn); cursor: pointer; }
  .remove:hover, .remove:focus-visible { background: var(--warn); color: var(--ink); }
  .remove[disabled] { opacity: 0.4; }
</style>
