<!--
  Sunucular sayfası (ana alanı kaplar; activity bar'daki sunucu simgesi). Kayıtlı sunucular kart kart; tek tıkla
  `ssh` terminali kendi penceresinde açılır. Sağda düzenleme çekmecesi (yeni/düzenle) canlı komut önizlemesiyle.
  Kartta ulaşılabilirlik (TCP, ms), açık bağlantı sayısı, grup rengi. Klavye: / ara, ↑↓←→ seç, ↩ bağlan, E düzenle,
  ⌘N yeni, Esc kapat. Parola/anahtar saklanmaz: kimlik doğrulamayı kullanıcının ssh'ı yapar.
-->
<script lang="ts">
  import { onMount, tick } from 'svelte'
  import { confirm } from './dialog.svelte'
  import { ago } from './format'
  import { menu } from './menu.svelte'
  import { now } from './now.svelte'
  import { blank, groupHue, servers, shellLine, sshCommand, target, type Server, type SshHost } from './servers.svelte'
  import { ui } from './ui.svelte'
  import * as api from '../term/api'

  let query = $state('')
  let group = $state<string | null>(null)
  /** Klavyeyle seçili kart */
  let sel = $state<string | null>(null)
  /** Düzenlenen sunucu (yeni için boş kimlik); null = çekmece kapalı */
  let edit = $state<Server | null>(null)
  let portText = $state('')
  let formError = $state<string | null>(null)
  let saving = $state(false)
  let copied = $state<string | null>(null)
  let searchEl = $state<HTMLInputElement>()
  let hostEl = $state<HTMLInputElement>()

  onMount(() => {
    void servers.refresh().then(() => servers.probe())
    // Ulaşılabilirlik sayfa açıkken 30 sn'de bir; açık bağlantılar oturum olaylarıyla
    const t = setInterval(() => !document.hidden && void servers.probe(), 30_000)
    void api.termState().then((s) => servers.track(s.sessions)).catch(() => {})
    const un = api.onSessions((s) => servers.track(s.sessions))
    return () => {
      clearInterval(t)
      void un.then((f) => f())
    }
  })

  const sorted = $derived(
    [...servers.list].sort((a, b) => (b.lastUsed ?? 0) - (a.lastUsed ?? 0) || a.name.localeCompare(b.name)),
  )
  const filtered = $derived.by(() => {
    const q = query.trim().toLowerCase()
    return sorted.filter(
      (s) =>
        (group === null || (s.group ?? '') === group) &&
        (!q || [s.name, s.host, s.user ?? '', s.group ?? ''].some((v) => v.toLowerCase().includes(q))),
    )
  })
  const groupCounts = $derived.by(() => {
    const m = new Map<string, number>()
    for (const s of servers.list) m.set(s.group ?? '', (m.get(s.group ?? '') ?? 0) + 1)
    return [...m.entries()].sort(([a], [b]) => (a === '' ? 1 : b === '' ? -1 : a.localeCompare(b)))
  })
  const knownGroups = $derived(groupCounts.map(([g]) => g).filter((g) => g))
  const online = $derived(Object.values(servers.reach).filter((r) => r.state === 'up').length)
  const connected = $derived(Object.keys(servers.open).length)
  /** ssh config'te olup henüz kaydedilmemiş takma adlar */
  const suggestions = $derived(servers.sshConfig.filter((h) => !servers.list.some((s) => s.host === h.alias)))

  // Seçim filtre dışında kaldıysa ilk karta
  $effect(() => {
    if (sel && !filtered.some((s) => s.id === sel)) sel = filtered[0]?.id ?? null
  })

  const preview = $derived.by(() => {
    if (!edit) return ''
    const port = Number(portText.trim())
    const host = edit.host.trim() || 'host'
    let user = edit.user?.trim() || null
    let h = host
    if (!user && host.includes('@')) [user, h] = host.split('@', 2) as [string, string]
    return shellLine(
      sshCommand({ host: h, user, port: Number.isInteger(port) && port > 0 ? port : null, identityFile: edit.identityFile?.trim() || null }),
    )
  })

  async function open(s: Server | null, base?: Partial<Server>) {
    edit = s ? { ...s } : { ...blank(), group: group || null, ...base }
    portText = edit.port ? String(edit.port) : ''
    formError = null
    await tick()
    hostEl?.focus()
    if (s) hostEl?.select()
  }

  async function save(andConnect: boolean) {
    if (!edit || saving) return
    const port = portText.trim() ? Number(portText.trim()) : null
    if (port !== null && (!Number.isInteger(port) || port < 1 || port > 65535)) {
      formError = 'port must be a number between 1 and 65535'
      return
    }
    saving = true
    const empty = (v: string | null) => (v && v.trim() ? v.trim() : null)
    formError = await servers.save({ ...edit, port, user: empty(edit.user), identityFile: empty(edit.identityFile), group: empty(edit.group) })
    saving = false
    if (formError) return
    edit = null
    const s = servers.saved
    if (s) {
      sel = s.id
      if (andConnect) void servers.connect(s)
      else void servers.probe()
    }
  }

  async function importHost(h: SshHost) {
    const err = await servers.save({ ...blank(), name: h.alias, host: h.alias })
    if (err) servers.error = err
  }

  async function importAll() {
    for (const h of suggestions) await importHost(h)
    void servers.probe()
  }

  function remove(s: Server) {
    confirm.ask({
      title: `remove ${s.name}?`,
      body: `Only removes it from this list. Nothing on ${target(s)} or in ~/.ssh is touched.`,
      action: 'remove',
      danger: true,
      run: async () => {
        await servers.remove(s.id)
        if (edit?.id === s.id) edit = null
      },
    })
  }

  async function copy(s: Server) {
    await navigator.clipboard.writeText(shellLine(sshCommand(s))).catch(() => {})
    copied = s.id
    setTimeout(() => copied === s.id && (copied = null), 1400)
  }

  function context(e: MouseEvent, s: Server) {
    e.preventDefault()
    sel = s.id
    const n = servers.open[s.id]?.length ?? 0
    menu.show(e.clientX, e.clientY, [
      { label: 'connect', action: () => void servers.connect(s) },
      ...(n ? [{ label: `show open session${n > 1 ? 's' : ''}`, action: () => void servers.focus(s) }] : []),
      { label: 'copy ssh command', action: () => void copy(s) },
      'sep',
      { label: 'edit…', action: () => void open(s) },
      { label: 'duplicate', action: () => void open(null, { ...s, id: '', name: `${s.name} copy`, lastUsed: null }) },
      'sep',
      { label: 'remove', danger: true, action: () => remove(s) },
    ])
  }

  /** Izgarada bir satırdaki kart sayısı (←→ yerine ↑↓ satır atlasın) */
  function columns(): number {
    const grid = document.querySelector('.srv .grid')
    if (!grid) return 1
    return getComputedStyle(grid).gridTemplateColumns.split(' ').length
  }

  function onkeydown(e: KeyboardEvent) {
    if (!ui.servers || confirm.open || menu.open || e.defaultPrevented) return
    const typing = e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement
    if (e.key === 'Escape') {
      e.preventDefault()
      if (edit) edit = null
      else if (typing && query) query = ''
      else if (typing) (e.target as HTMLElement).blur()
      else ui.servers = false
      return
    }
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'n') {
      e.preventDefault()
      void open(null)
      return
    }
    if (edit) {
      if (e.key === 'Enter' && typing) {
        e.preventDefault()
        void save(e.metaKey || e.ctrlKey)
      }
      return
    }
    if (typing && e.target !== searchEl) return
    const i = filtered.findIndex((s) => s.id === sel)
    const move = (d: number) => {
      e.preventDefault()
      const next = filtered[Math.max(0, Math.min(filtered.length - 1, i < 0 ? 0 : i + d))]
      if (next) {
        sel = next.id
        document.getElementById(`srv-${next.id}`)?.scrollIntoView({ block: 'nearest' })
      }
    }
    if (e.key === 'ArrowDown') return move(columns())
    if (e.key === 'ArrowUp') return move(-columns())
    if (e.key === 'ArrowRight' && !typing) return move(1)
    if (e.key === 'ArrowLeft' && !typing) return move(-1)
    const cur = filtered[i] ?? (typing ? filtered[0] : undefined)
    if (e.key === 'Enter' && cur) {
      e.preventDefault()
      void servers.connect(cur)
      return
    }
    if (typing) return
    if (e.key === '/') {
      e.preventDefault()
      searchEl?.focus()
    } else if (e.key.toLowerCase() === 'e' && cur) {
      e.preventDefault()
      void open(cur)
    } else if ((e.key === 'Backspace' || e.key === 'Delete') && cur) {
      e.preventDefault()
      remove(cur)
    }
  }
</script>

<svelte:window onkeydowncapture={onkeydown} />

<section class="page srv" aria-label="servers">
  <header>
    <span class="label">servers</span>
    <span class="stats" aria-live="polite">
      <b>{servers.list.length}</b> saved
      {#if servers.list.length}· <span class="up">{online} reachable</span>{/if}
      {#if connected}· <span class="conn">{connected} connected</span>{/if}
    </span>
    <span class="grow"></span>
    <label class="search">
      <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="11" cy="11" r="6" /><path d="m20 20-4.2-4.2" /></svg>
      <input bind:this={searchEl} type="search" placeholder="filter" aria-label="filter servers" bind:value={query} spellcheck="false" autocapitalize="off" />
      {#if !query}<kbd>/</kbd>{/if}
    </label>
    <button class="hbtn" title="check reachability" aria-label="check reachability" onclick={() => servers.probe()}>↻</button>
    <button class="hbtn add" onclick={() => open(null)}>+ new server <kbd>⌘N</kbd></button>
    <button class="close" title="close (Esc)" aria-label="close servers" onclick={() => (ui.servers = false)}>×</button>
  </header>

  <div class="main" class:drawer={!!edit}>
    <div class="body">
      {#if servers.error}
        <div class="err" role="alert">⚠ {servers.error}<button class="x" aria-label="dismiss" onclick={() => (servers.error = null)}>×</button></div>
      {/if}

      {#if groupCounts.length > 1}
        <div class="chips" role="tablist" aria-label="groups">
          <button role="tab" aria-selected={group === null} class:on={group === null} onclick={() => (group = null)}>all <span>{servers.list.length}</span></button>
          {#each groupCounts as [g, n] (g)}
            <button role="tab" aria-selected={group === g} class:on={group === g} style:--hue={g ? groupHue(g) : 'var(--dim)'} onclick={() => (group = group === g ? null : g)}>
              <i aria-hidden="true"></i>{g || 'ungrouped'} <span>{n}</span>
            </button>
          {/each}
        </div>
      {/if}

      {#if servers.loaded && !servers.list.length}
        <div class="empty">
          <svg viewBox="0 0 64 64" aria-hidden="true">
            <rect x="10" y="12" width="44" height="16" rx="2" /><rect x="10" y="36" width="44" height="16" rx="2" />
            <path d="M17 20h.01M17 44h.01M24 20h18M24 44h18" />
          </svg>
          <h2>No servers yet</h2>
          <p>Save a server once, then connect with a single click. Your own <code>ssh</code> does the login, so keys,
            agents and <code>~/.ssh/config</code> work as usual.</p>
          <div class="row">
            <button class="primary" onclick={() => open(null)}>+ new server</button>
            {#if suggestions.length}
              <button class="ghost" onclick={importAll}>import {suggestions.length} from ~/.ssh/config</button>
            {/if}
          </div>
        </div>
      {:else if servers.list.length && !filtered.length}
        <div class="none">
          No server matches{query ? ` "${query}"` : ''}{group !== null ? ` in ${group || 'ungrouped'}` : ''}.
          <button class="link" onclick={() => { query = ''; group = null }}>clear filters</button>
        </div>
      {/if}

      <div class="grid" role="list">
        {#each filtered as s (s.id)}
          {@const r = servers.reach[s.id]}
          {@const n = servers.open[s.id]?.length ?? 0}
          <div
            id="srv-{s.id}"
            role="listitem"
            class="card"
            class:sel={sel === s.id}
            class:busy={servers.connecting === s.id}
            style:--hue={s.group ? groupHue(s.group) : 'var(--line)'}
            oncontextmenu={(e) => context(e, s)}
            onmouseenter={() => (sel = s.id)}
          >
            <button class="hit" title="connect · ssh {target(s)}" onclick={() => servers.connect(s)}>
              <span class="top">
                <span class="dot {r?.state ?? 'checking'}" aria-hidden="true"></span>
                <span class="name">{s.name}</span>
                {#if n}<span class="open" title="{n} open session{n > 1 ? 's' : ''}">● {n} open</span>{/if}
              </span>
              <span class="tgt">{target(s)}</span>
              <span class="meta">
                <span class="reach {r?.state ?? 'checking'}">
                  {#if servers.connecting === s.id}connecting…
                  {:else if !r || r.state === 'checking'}{r?.ms != null ? `${r.ms} ms` : 'checking…'}
                  {:else if r.state === 'up'}{r.ms} ms
                  {:else if r.state === 'down'}unreachable
                  {:else}via proxy{/if}
                </span>
                <span class="sep">·</span>
                <span>{s.lastUsed ? `${ago(now.value - s.lastUsed)} ago` : 'never used'}</span>
                {#if s.identityFile}<span class="sep">·</span><span title={s.identityFile}>key</span>{/if}
              </span>
            </button>
            <div class="tools">
              {#if n}<button title="show open session" aria-label="show open session of {s.name}" onclick={() => servers.focus(s)}>⧉</button>{/if}
              <button title={copied === s.id ? 'copied' : 'copy ssh command'} aria-label="copy ssh command for {s.name}" onclick={() => copy(s)}>{copied === s.id ? '✓' : '⎘'}</button>
              <button title="edit (E)" aria-label="edit {s.name}" onclick={() => open(s)}>✎</button>
              <button class="del" title="remove" aria-label="remove {s.name}" onclick={() => remove(s)}>✕</button>
            </div>
            <span class="go" aria-hidden="true">connect ↩</span>
          </div>
        {/each}
        {#if servers.list.length && filtered.length}
          <button class="card ghostcard" onclick={() => open(null)}><span>+ new server</span></button>
        {/if}
      </div>

      {#if suggestions.length && servers.list.length}
        <div class="sugg">
          <div class="shead">
            <h3>from ~/.ssh/config <span class="n">{suggestions.length}</span></h3>
            <span class="grow"></span>
            <button class="link" onclick={importAll}>add all</button>
          </div>
          {#each suggestions as h (h.alias)}
            <div class="srow">
              <span class="name">{h.alias}</span>
              <span class="tgt">{target({ host: h.hostName ?? h.alias, user: h.user, port: h.port })}{h.proxy ? ' · via proxy' : ''}</span>
              <button class="ghost sm" onclick={() => importHost(h)}>+ add</button>
            </div>
          {/each}
          <p class="note">Read-only. These connect with the settings in your ssh config.</p>
        </div>
      {/if}

      {#if servers.list.length}
        <p class="keys"><kbd>↩</kbd> connect <kbd>↑↓←→</kbd> select <kbd>E</kbd> edit <kbd>/</kbd> filter <kbd>⌘N</kbd> new · right-click for more</p>
      {/if}
    </div>

    {#if edit}
      <aside class="editor" aria-label={edit.id ? 'edit server' : 'new server'}>
        <form onsubmit={(e) => { e.preventDefault(); void save(false) }}>
          <div class="ehead">
            <h2>{edit.id ? 'edit server' : 'new server'}</h2>
            <button type="button" class="x" aria-label="close editor" onclick={() => (edit = null)}>×</button>
          </div>

          <h3>connection</h3>
          <label class="f">
            <span>host <em>required</em></span>
            <input bind:this={hostEl} bind:value={edit.host} placeholder="example.com, 10.0.0.5 or an ssh config alias" required spellcheck="false" autocapitalize="off" autocomplete="off" />
          </label>
          <div class="two">
            <label class="f">
              <span>user</span>
              <input bind:value={edit.user} placeholder="ssh default" spellcheck="false" autocapitalize="off" autocomplete="off" />
            </label>
            <label class="f port">
              <span>port</span>
              <input bind:value={portText} placeholder="22" inputmode="numeric" autocomplete="off" />
            </label>
          </div>

          <h3>organize</h3>
          <label class="f">
            <span>name</span>
            <input bind:value={edit.name} placeholder={edit.host.trim() || 'defaults to host'} spellcheck="false" autocomplete="off" />
          </label>
          <label class="f">
            <span>group</span>
            <input bind:value={edit.group} placeholder="prod, staging…" spellcheck="false" autocapitalize="off" autocomplete="off" />
          </label>
          {#if knownGroups.length}
            <div class="gpick">
              {#each knownGroups as g (g)}
                <button type="button" class:on={edit.group === g} style:--hue={groupHue(g)} onclick={() => edit && (edit.group = edit.group === g ? null : g)}><i aria-hidden="true"></i>{g}</button>
              {/each}
            </div>
          {/if}

          <h3>authentication</h3>
          <label class="f">
            <span>key file</span>
            <input bind:value={edit.identityFile} placeholder="~/.ssh/id_ed25519" spellcheck="false" autocapitalize="off" autocomplete="off" />
          </label>
          <p class="note">Leave empty to use ssh-agent and ~/.ssh/config. Passwords and keys are never stored; ssh asks in the terminal when it needs one.</p>

          <div class="cmd" aria-label="command preview">
            <span class="p">$</span><code>{preview}</code>
          </div>

          {#if formError}<div class="err" role="alert">⚠ {formError}</div>{/if}

          <button type="button" class="primary wide" disabled={saving} onclick={() => save(true)}>{saving ? 'saving…' : 'save & connect'} <kbd>⌘↩</kbd></button>
          <div class="actions">
            {#if edit.id}
              {@const cur = servers.list.find((x) => x.id === edit?.id)}
              {#if cur}<button type="button" class="link danger" onclick={() => remove(cur)}>remove</button>{/if}
            {/if}
            <span class="grow"></span>
            <button type="button" class="ghost" onclick={() => (edit = null)}>cancel <kbd>esc</kbd></button>
            <button type="submit" class="ghost" disabled={saving}>save <kbd>↩</kbd></button>
          </div>
        </form>
      </aside>
    {/if}
  </div>
</section>

<style>
  .page {
    height: 100%;
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: rgb(var(--bg-rgb));
    border: 1px solid var(--line-soft);
    animation: fade 0.16s ease-out;
  }
  @keyframes fade { from { opacity: 0; } }
  header { display: flex; align-items: center; gap: 10px; height: 22px; border-bottom: 1px solid var(--line-soft); }
  .label { height: 100%; display: flex; align-items: center; padding: 0 8px; background: var(--accent); color: var(--ink); font-weight: 700; letter-spacing: 0.04em; }
  .stats { color: var(--dim); white-space: nowrap; }
  .stats b { color: var(--hi); }
  .stats .up { color: var(--green); }
  .stats .conn { color: var(--accent); }
  .grow { flex: 1; }
  kbd {
    font: inherit;
    font-size: 0.8em;
    padding: 0 4px;
    border: 1px solid var(--line);
    color: var(--dim);
    line-height: 1.3;
  }
  .search {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 220px;
    height: 18px;
    padding: 0 6px;
    border: 1px solid var(--line-soft);
    color: var(--dim);
  }
  .search:focus-within { border-color: var(--accent); color: var(--accent); }
  .search svg { width: 12px; height: 12px; flex: none; fill: none; stroke: currentColor; stroke-width: 2; }
  .search input { all: unset; flex: 1; min-width: 0; color: var(--hi); font: inherit; }
  .search input::placeholder { color: var(--dim); }
  .hbtn { all: unset; padding: 0 6px; color: var(--dim); cursor: pointer; white-space: nowrap; }
  .hbtn:hover, .hbtn:focus-visible { color: var(--hi); }
  .hbtn.add { color: var(--accent); font-weight: 700; }
  .hbtn.add kbd { color: var(--accent); border-color: rgba(var(--accent-rgb), 0.4); }
  .close { all: unset; padding: 0 10px; color: var(--dim); cursor: pointer; }
  .close:hover, .close:focus-visible { color: var(--hi); }

  .main { flex: 1; min-height: 0; display: grid; grid-template-columns: 1fr; }
  .main.drawer { grid-template-columns: 1fr 360px; }
  .body { min-height: 0; overflow: auto; padding: 12px 16px 24px; }

  .err { display: flex; align-items: center; gap: 8px; color: var(--red); margin: 0 0 10px; padding: 4px 8px; border: 1px solid rgba(var(--red-rgb), 0.4); background: rgba(var(--red-rgb), 0.08); }
  .err .x { margin-left: auto; }
  .x { all: unset; color: var(--dim); cursor: pointer; padding: 0 4px; }
  .x:hover { color: var(--hi); }

  .chips { display: flex; flex-wrap: wrap; gap: 6px; margin-bottom: 12px; }
  .chips button {
    all: unset;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 1px 8px;
    border: 1px solid var(--line-soft);
    color: var(--txt2);
    cursor: pointer;
  }
  .chips button span { color: var(--dim); }
  .chips button:hover, .chips button:focus-visible { border-color: var(--line); color: var(--hi); }
  .chips button.on { border-color: var(--accent); color: var(--hi); background: rgba(var(--accent-rgb), 0.1); }
  .chips i, .gpick i { width: 7px; height: 7px; background: var(--hue); flex: none; }

  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(250px, 1fr)); gap: 10px; }
  .card {
    position: relative;
    border: 1px solid var(--line-soft);
    border-left: 3px solid var(--hue);
    background: rgba(var(--panel-rgb), 0.9);
    transition: border-color 0.12s, box-shadow 0.12s, transform 0.12s;
  }
  .card.sel { border-color: rgba(var(--accent-rgb), 0.6); border-left-color: var(--hue); box-shadow: 0 0 12px rgba(var(--accent-rgb), 0.18); }
  .card.busy { border-color: var(--accent); }
  .hit {
    all: unset;
    box-sizing: border-box;
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 10px 12px 10px;
    cursor: pointer;
  }
  .hit:focus-visible { outline: 1px solid var(--accent); outline-offset: -1px; }
  .hit:active { background: rgba(var(--accent-rgb), 0.06); }
  .top { display: flex; align-items: center; gap: 8px; min-width: 0; padding-right: 76px; }
  .name { color: var(--hi); font-weight: 700; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tgt { color: var(--txt2); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; padding-left: 16px; }
  .meta { display: flex; gap: 6px; color: var(--dim); font-size: 0.85em; padding-left: 16px; white-space: nowrap; overflow: hidden; }
  .sep { color: var(--faint); }
  .open { flex: none; color: var(--accent); font-size: 0.85em; border: 1px solid rgba(var(--accent-rgb), 0.45); padding: 0 5px; }

  .dot { width: 8px; height: 8px; border-radius: 50%; flex: none; background: var(--faint); }
  .dot.up { background: var(--green); box-shadow: 0 0 6px rgba(var(--green-rgb), 0.7); }
  .dot.down { background: var(--red); }
  .dot.unknown { background: transparent; border: 1px solid var(--dim); box-sizing: border-box; }
  .dot.checking { animation: pulse 1s ease-in-out infinite; }
  @keyframes pulse { 50% { opacity: 0.3; } }
  .reach.up { color: var(--green); }
  .reach.down { color: var(--red); }
  .busy .reach { color: var(--accent); }

  .tools { position: absolute; top: 6px; right: 6px; display: flex; gap: 2px; opacity: 0; transition: opacity 0.12s; }
  .card:hover .tools, .card:focus-within .tools, .card.sel .tools { opacity: 1; }
  .tools button { all: unset; width: 20px; height: 18px; text-align: center; color: var(--dim); cursor: pointer; }
  .tools button:hover, .tools button:focus-visible { color: var(--hi); background: rgba(var(--accent-rgb), 0.1); }
  .tools .del:hover { color: var(--red); }
  .go { position: absolute; right: 10px; bottom: 8px; color: var(--accent); font-size: 0.8em; opacity: 0; transition: opacity 0.12s; pointer-events: none; }
  .card:hover .go, .card.sel .go { opacity: 1; }
  .busy .go { opacity: 0 !important; }

  .ghostcard {
    all: unset;
    box-sizing: border-box;
    min-height: 74px;
    display: grid;
    place-items: center;
    border: 1px dashed var(--line);
    color: var(--dim);
    cursor: pointer;
  }
  .ghostcard:hover, .ghostcard:focus-visible { border-color: var(--accent); color: var(--accent); }

  .empty { display: flex; flex-direction: column; align-items: center; text-align: center; gap: 10px; padding: 56px 16px; color: var(--dim); }
  .empty svg { width: 64px; height: 64px; fill: none; stroke: var(--accent); stroke-width: 1.4; stroke-linecap: round; opacity: 0.8; }
  .empty h2 { margin: 0; color: var(--hi); font-size: 1.1em; }
  .empty p { margin: 0; max-width: 440px; line-height: 1.5; }
  .empty .row { display: flex; gap: 8px; margin-top: 6px; }
  code { color: var(--txt2); }
  .none { color: var(--dim); padding: 24px 0; text-align: center; }

  h3 { margin: 0; color: var(--dim); font-size: 0.85em; font-weight: 600; text-transform: uppercase; letter-spacing: 0.05em; }
  .n { color: var(--accent); margin-left: 4px; }
  .note { color: var(--dim); font-size: 0.85em; margin: 6px 0 0; line-height: 1.45; }
  .keys { color: var(--faint); font-size: 0.85em; margin: 22px 0 0; display: flex; flex-wrap: wrap; gap: 6px; align-items: center; }
  .keys kbd { color: var(--dim); }

  .sugg { margin-top: 22px; }
  .shead { display: flex; align-items: center; margin-bottom: 6px; }
  .srow { display: grid; grid-template-columns: minmax(120px, 200px) 1fr auto; gap: 10px; align-items: center; padding: 5px 8px; border: 1px solid var(--line-soft); }
  .srow + .srow { border-top: 0; }
  .srow .name { font-weight: 400; color: var(--txt); }
  .srow .tgt { padding: 0; color: var(--dim); }

  .primary, .ghost, .link { all: unset; cursor: pointer; white-space: nowrap; }
  .primary, .ghost { padding: 3px 12px; font-weight: 700; display: inline-flex; align-items: center; gap: 6px; }
  .primary { background: var(--accent); color: var(--ink); }
  .primary kbd { color: var(--ink); border-color: rgba(var(--bg-rgb), 0.35); }
  .primary:hover, .primary:focus-visible { filter: brightness(1.15); }
  .ghost { border: 1px solid var(--line); color: var(--txt2); }
  .ghost:hover, .ghost:focus-visible { border-color: var(--accent); color: var(--hi); }
  .ghost.sm { padding: 0 8px; font-weight: 400; }
  .primary[disabled], .ghost[disabled] { opacity: 0.5; cursor: default; }
  .link { color: var(--accent); }
  .link:hover, .link:focus-visible { text-decoration: underline; }
  .link.danger { color: var(--red); }

  .editor {
    min-height: 0;
    overflow: auto;
    border-left: 1px solid var(--line);
    background: rgba(var(--panel-rgb), 0.95);
    animation: slide 0.16s ease-out;
  }
  @keyframes slide { from { transform: translateX(16px); opacity: 0; } }
  .editor form { display: flex; flex-direction: column; gap: 8px; padding: 12px 14px 16px; }
  .ehead { display: flex; align-items: center; }
  .ehead h2 { margin: 0; flex: 1; color: var(--accent); font-size: 1em; }
  .editor h3 { margin-top: 8px; padding-bottom: 3px; border-bottom: 1px solid var(--line-soft); }
  .f { display: flex; flex-direction: column; gap: 3px; }
  .f span { color: var(--txt3); font-size: 0.85em; }
  .f em { color: var(--faint); font-style: normal; margin-left: 4px; }
  .f input {
    all: unset;
    padding: 4px 7px;
    border: 1px solid var(--line);
    background: rgba(var(--bg-rgb), 0.6);
    color: var(--hi);
    font: inherit;
  }
  .f input::placeholder { color: var(--faint); }
  .f input:focus { border-color: var(--accent); box-shadow: 0 0 0 1px rgba(var(--accent-rgb), 0.3); }
  .two { display: grid; grid-template-columns: 1fr 76px; gap: 8px; }
  .gpick { display: flex; flex-wrap: wrap; gap: 4px; }
  .gpick button { all: unset; display: flex; align-items: center; gap: 5px; padding: 0 6px; border: 1px solid var(--line-soft); color: var(--dim); cursor: pointer; font-size: 0.85em; }
  .gpick button:hover { color: var(--hi); }
  .gpick button.on { color: var(--hi); border-color: var(--accent); }
  .cmd {
    display: flex;
    gap: 8px;
    margin-top: 6px;
    padding: 7px 9px;
    border: 1px solid var(--line-soft);
    background: rgba(var(--term-bg-rgb), 0.9);
    line-height: 1.45;
  }
  .cmd .p { color: var(--accent); }
  .cmd .p { flex: none; }
  .cmd code { color: var(--hi); white-space: pre-wrap; word-break: break-all; }
  .actions { display: flex; align-items: center; gap: 6px; }
  .wide { justify-content: center; margin-top: 8px; padding: 6px 12px; }

  @media (max-width: 760px) {
    .main.drawer { grid-template-columns: 1fr; }
    .main.drawer .body { display: none; }
    .search { width: 140px; }
    .stats { display: none; }
  }
  @media (prefers-reduced-motion: reduce) {
    .page, .editor, .dot.checking { animation: none; }
    .card, .tools, .go { transition: none; }
  }
</style>
