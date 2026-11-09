<!--
  Source Control (OZELLIKLER.md §4): sohbetlerin ve terminallerin çalıştığı her repo. Commit mesajı, Commit /
  Commit & Push / Commit & Sync, Staged Changes ve Changes, geçmiş grafiği. Sol sütunda sohbet listesinin yerine
  açılır (activity bar ya da ⌃⇧G); Esc sohbet listesine döner.
-->
<script lang="ts">
  import { untrack } from 'svelte'
  import BranchPicker from './BranchPicker.svelte'
  import FileIcon from './code/FileIcon.svelte'
  import { confirm } from './dialog.svelte'
  import { ago } from './format'
  import { git } from './git.svelte'
  import { layoutGraph } from './gitGraph'
  import { now } from './now.svelte'
  import type { GitChange, RepoStatus } from './types'
  import { ui } from './ui.svelte'
  import * as api from '../term/api'

  const ROW = 22
  const LANE = 11

  let collapsed = $state<Record<string, boolean>>({})
  let historyOpen = $state<Record<string, boolean>>({})
  let menuFor = $state<string | null>(null)
  /** Dal seçicisi açık repo */
  let branchFor = $state<string | null>(null)

  // Açıkken sık yoklama. untrack: refresh'in okuduğu snapshot bu efekti her saniye yeniden çalıştırmasın.
  $effect(() => {
    untrack(() => git.setVisible(true))
    return () => untrack(() => git.setVisible(false))
  })

  const CODE_LABEL: Record<string, string> = {
    M: 'modified', A: 'added', D: 'deleted', R: 'renamed', C: 'copied', T: 'type changed', U: 'conflict', '?': 'untracked',
  }

  function split(path: string) {
    const i = path.lastIndexOf('/')
    return i < 0 ? { name: path, dir: '' } : { name: path.slice(i + 1), dir: path.slice(0, i) }
  }

  function onkeydown(e: KeyboardEvent) {
    if (!ui.scm || e.defaultPrevented || ui.code) return
    if (e.key === 'Escape' && !confirm.open) {
      if (menuFor) menuFor = null
      else if (branchFor) branchFor = null
      else ui.scm = false
    }
  }

  function commitWith(st: RepoStatus, then: 'none' | 'push' | 'sync') {
    menuFor = null
    const root = st.root
    if (!(git.messages[root] ?? '').trim()) {
      git.errors[root] = 'write a commit message first'
      return
    }
    if (st.staged.length === 0) {
      if (st.changes.length === 0) {
        git.errors[root] = 'nothing to commit'
        return
      }
      confirm.ask({
        title: 'No staged changes',
        body: `Stage all ${st.changes.length} changes and commit them?`,
        action: 'stage all & commit',
        run: () => git.commit(root, then, true),
      })
      return
    }
    void git.commit(root, then)
  }

  function discard(root: string, files: GitChange[]) {
    const tracked = files.filter((c) => c.code !== '?')
    if (!tracked.length) return
    confirm.ask({
      title: tracked.length === 1 ? `Discard changes in ${split(tracked[0].path).name}?` : `Discard ${tracked.length} changes?`,
      body: 'The working-tree changes are lost. Untracked files are left alone.',
      action: 'discard',
      danger: true,
      run: () => void git.run(root, 'discard', tracked.map((c) => c.path)),
    })
  }

  /** Uzak deponun adı (upstream'in ilk parçası; yoksa origin): yerel dal adında da `/` olabilir. */
  const remoteOf = (st: RepoStatus) => st.upstream?.split('/')[0] ?? 'origin'

  /** Grafik: şerit x'i */
  const lx = (i: number) => 6 + i * LANE
</script>

<svelte:window {onkeydown} />

<aside class="scm" aria-label="source control">
    <header>
      <span class="label">source.control</span>
      <span class="sub">{git.repos.length} repo{git.repos.length === 1 ? '' : 's'}</span>
      <span class="grow"></span>
      <button class="ic" title="refresh" aria-label="refresh" onclick={() => git.refresh(true)}>↻</button>
      <button class="ic" title="back to chats (Esc)" aria-label="close" onclick={() => (ui.scm = false)}>×</button>
    </header>

    <div class="body">
      {#if git.repos.length === 0}
        <p class="empty">No repositories yet. Repos show up here when a chat or terminal works inside one.</p>
      {/if}

      {#each git.repos as repo (repo.root)}
        {@const st = git.status[repo.root]}
        {@const busy = git.busy[repo.root]}
        {@const open = !collapsed[repo.root]}
        {@const dirty = !!st && st.staged.length + st.changes.length + st.conflicts.length > 0}
        <section class="repo">
          <!-- Repo satırı (VS Code gibi): ad · dal (değişiklik varsa *) · senkron sayaçları -->
          <div class="rhead">
            <button class="twist" aria-expanded={open} onclick={() => (collapsed[repo.root] = open)}>
              <span class="caret">{open ? '▾' : '▸'}</span>
              <svg class="rico" viewBox="0 0 16 16" aria-hidden="true"><rect x="2.5" y="1.5" width="11" height="13" rx="1.5" /><path d="M5 1.5v13M7.5 5h3.5M7.5 8h3.5" /></svg>
              <b>{repo.name}</b>
            </button>
            <span class="grow"></span>
            {#if busy}<span class="busy">{busy}…</span>{/if}
            {#if st}
              <button
                class="branch"
                data-branch-trigger
                disabled={!!busy}
                aria-haspopup="dialog"
                aria-expanded={branchFor === repo.root}
                title="{st.branch}{st.upstream ? ` · tracking ${st.upstream}` : ' · no upstream'} — switch or create a branch"
                onclick={() => (branchFor = branchFor === repo.root ? null : repo.root)}>⑂ {st.branch}{dirty ? '*' : ''}</button
              >
              {#if st.upstream}
                <button
                  class="ab"
                  disabled={!!busy}
                  title="sync: pull {st.behind}, push {st.ahead}"
                  aria-label="sync ({st.behind} to pull, {st.ahead} to push)"
                  onclick={() => git.run(repo.root, 'sync')}>↻ {st.behind}↓ {st.ahead}↑</button
                >
              {/if}
            {/if}
            {#if branchFor === repo.root && st}
              <BranchPicker root={repo.root} current={st.branch} onclose={() => (branchFor = null)} />
            {/if}
          </div>
          {#if git.errors[repo.root]}<div class="err" role="alert">✕ {git.errors[repo.root]}</div>{/if}

          {#if open && st}
            <div class="msg">
              <textarea
                rows={Math.min(8, Math.max(2, (git.messages[repo.root] ?? '').split('\n').length))}
                readonly={git.writing[repo.root]}
                placeholder="Message (⌘Enter to commit on &quot;{st.branch}&quot;)"
                aria-label="commit message for {repo.name}"
                bind:value={git.messages[repo.root]}
                onkeydown={(e) => {
                  if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
                    e.preventDefault()
                    commitWith(st, 'none')
                  }
                }}
              ></textarea>
              <button
                class="ai"
                class:writing={git.writing[repo.root]}
                disabled={git.writing[repo.root] || !dirty}
                title="Write the message with your Claude Code (claude -p, uses your own sign-in / subscription; the diff is sent to Claude)"
                aria-label="generate commit message with Claude"
                onclick={() => git.aiMessage(repo.root)}>{git.writing[repo.root] ? '…' : '✧'}</button
              >
            </div>
            <!-- Tek büyük düğme: değişiklik varsa Commit (▾ ile & Push / & Sync), yoksa senkron ya da yayınla -->
            <div class="actions">
              {#if dirty}
                <button class="primary" disabled={!!busy} onclick={() => commitWith(st, 'none')}>✓ Commit</button>
                <button class="more" disabled={!!busy} aria-haspopup="menu" aria-expanded={menuFor === repo.root} aria-label="more commit options" onclick={() => (menuFor = menuFor === repo.root ? null : repo.root)}>▾</button>
              {:else if !st.upstream && !st.detached && !st.initial}
                <button class="primary" disabled={!!busy} title="push and set upstream" onclick={() => git.run(repo.root, 'push')}>⇡ Publish Branch</button>
              {:else if st.ahead || st.behind}
                <button class="primary" disabled={!!busy} title="pull, then push" onclick={() => git.run(repo.root, 'sync')}>↻ Sync Changes{st.behind ? ` ${st.behind}↓` : ''}{st.ahead ? ` ${st.ahead}↑` : ''}</button>
              {:else}
                <button class="primary" disabled>✓ Commit</button>
              {/if}
              {#if menuFor === repo.root}
                <div class="menu" role="menu">
                  <button role="menuitem" onclick={() => commitWith(st, 'none')}>Commit</button>
                  <button role="menuitem" onclick={() => commitWith(st, 'push')}>Commit &amp; Push</button>
                  <button role="menuitem" onclick={() => commitWith(st, 'sync')}>Commit &amp; Sync</button>
                </div>
              {/if}
            </div>

            {#snippet list(title: string, files: GitChange[], kind: 'conflicts' | 'staged' | 'changes')}
              {#if files.length}
                <div class="lhead">
                  <span>{title}</span>
                  <span class="grow"></span>
                  {#if kind === 'staged'}
                    <button class="ic" title="unstage all" aria-label="unstage all" disabled={!!busy} onclick={() => git.run(repo.root, 'unstage', files.map((c) => c.path))}>−</button>
                  {:else}
                    {#if kind === 'changes'}
                      <button class="ic" title="discard all tracked changes" aria-label="discard all" disabled={!!busy} onclick={() => discard(repo.root, files)}>↺</button>
                    {/if}
                    <button class="ic" title="stage all" aria-label="stage all" disabled={!!busy} onclick={() => git.run(repo.root, 'stage', files.map((c) => c.path))}>+</button>
                  {/if}
                  <span class="count">{files.length}</span>
                </div>
                <ul>
                  {#each files as f (f.path)}
                    {@const p = split(f.path)}
                    <li class="file" title="{f.orig ? `${f.orig} → ` : ''}{f.path} · {CODE_LABEL[f.code] ?? f.code}">
                      <button
                        class="fname"
                        class:open={ui.code?.root === repo.root && ui.code.path === f.path && ui.code.staged === (kind === 'staged')}
                        title="open diff · double-click opens the file in its app"
                        onclick={() => (ui.code = { root: repo.root, path: f.path, orig: f.orig, staged: kind === 'staged', mode: 'diff' })}
                        ondblclick={() => api.openPath(`${repo.root}/${f.path}`, null)}
                      >
                        <FileIcon path={f.path} />
                        <span class="n" class:del={f.code === 'D'}>{p.name}</span>
                        {#if git.isEdited(repo.root, f.path)}<span class="star" title="edited by a chat today">✦</span>{/if}
                        <span class="d">{p.dir}</span>
                      </button>
                      <span class="fact">
                        {#if kind === 'staged'}
                          <button class="ic" title="unstage" aria-label="unstage {f.path}" disabled={!!busy} onclick={() => git.run(repo.root, 'unstage', [f.path])}>−</button>
                        {:else}
                          {#if kind === 'changes' && f.code !== '?'}
                            <button class="ic" title="discard changes" aria-label="discard {f.path}" disabled={!!busy} onclick={() => discard(repo.root, [f])}>↺</button>
                          {/if}
                          <button class="ic" title="stage" aria-label="stage {f.path}" disabled={!!busy} onclick={() => git.run(repo.root, 'stage', [f.path])}>+</button>
                        {/if}
                      </span>
                      <span class="code c{f.code === '?' ? 'Q' : f.code}" aria-label={CODE_LABEL[f.code] ?? f.code}>{f.code === '?' ? 'U' : f.code}</span>
                    </li>
                  {/each}
                </ul>
              {/if}
            {/snippet}

            {@render list('merge conflicts', st.conflicts, 'conflicts')}
            {@render list('staged changes', st.staged, 'staged')}
            {@render list('changes', st.changes, 'changes')}
            {#if st.truncated}<p class="note">Showing the first 2000 files.</p>{/if}
            {#if !st.staged.length && !st.changes.length && !st.conflicts.length}
              <p class="note">No changes.</p>
            {/if}

            <!-- Geçmiş grafiği: HEAD ve upstream -->
            {@const log = git.logs[repo.root] ?? []}
            <button class="lhead hist" aria-expanded={!!historyOpen[repo.root]} onclick={() => (historyOpen[repo.root] = !historyOpen[repo.root])}>
              <span>{historyOpen[repo.root] ? '▾' : '▸'} history</span><span class="count">{log.length}</span>
            </button>
            {#if historyOpen[repo.root] && log.length}
              {@const g = layoutGraph(log)}
              <ol class="graph">
                {#each g.rows as r (r.commit.hash)}
                  <li style:height="{ROW}px" title="{r.commit.hash.slice(0, 8)} · {r.commit.author}">
                    <svg width={lx(g.width) + 4} height={ROW} aria-hidden="true">
                      {#each r.above as h, i (i)}
                        {#if h === r.commit.hash}
                          <line x1={lx(i)} y1="0" x2={lx(r.col)} y2={ROW / 2} class="l{i % 6}" />
                        {:else if h}
                          {@const j = r.below.indexOf(h)}
                          {#if j >= 0}<line x1={lx(i)} y1="0" x2={lx(j)} y2={ROW} class="l{i % 6}" />{/if}
                        {/if}
                      {/each}
                      {#each r.commit.parents as p (p)}
                        {@const j = r.below.indexOf(p)}
                        {#if j >= 0}<line x1={lx(r.col)} y1={ROW / 2} x2={lx(j)} y2={ROW} class="l{j % 6}" />{/if}
                      {/each}
                      <circle cx={lx(r.col)} cy={ROW / 2} r={r.commit.refs.some((x) => x.startsWith('HEAD')) ? 4 : 3} class="d{r.col % 6}" class:head={r.commit.refs.some((x) => x.startsWith('HEAD'))} />
                    </svg>
                    <span class="subj">{r.commit.subject}</span>
                    {#each r.commit.refs as ref (ref)}
                      <span class="ref" class:headref={ref.startsWith('HEAD')} class:tag={ref.startsWith('tag:')} class:remote={!ref.startsWith('HEAD') && ref.startsWith(`${remoteOf(st)}/`)}>{ref.replace('HEAD -> ', '')}</span>
                    {/each}
                    <span class="when">{ago(now.value - r.commit.time)}</span>
                  </li>
                {/each}
              </ol>
            {/if}
          {:else if open && !st && !git.errors[repo.root]}
            <p class="note">loading…</p>
          {/if}
        </section>
      {/each}
    </div>
</aside>

<style>
  .scm {
    height: 100%;
    display: flex;
    flex-direction: column;
    background: rgba(var(--panel-rgb), 0.92);
    border: 1px solid var(--line-soft);
    animation: slide 0.16s ease-out;
  }
  @keyframes slide { from { transform: translateX(-8px); opacity: 0; } }
  @media (prefers-reduced-motion: reduce) { .scm { animation: none; } }
  header { display: flex; align-items: center; gap: 8px; height: 22px; padding-left: 0; border-bottom: 1px solid var(--line-soft); }
  .label { height: 100%; display: flex; align-items: center; padding: 0 8px; background: var(--accent); color: var(--ink); font-weight: 700; letter-spacing: 0.04em; }
  .sub { color: var(--dim); }
  .grow { flex: 1; }
  .body { flex: 1; overflow: auto; padding: 4px 0 12px; }
  .empty, .note { color: var(--faint); margin: 8px 10px; }

  /* Satır ve başlık düğmeleri (+ stage, − unstage, ↺ geri al): okunur boyutta, üzerine gelince çerçeveli */
  .ic {
    all: unset;
    flex: none;
    width: 22px;
    height: 20px;
    display: grid;
    place-items: center;
    font-size: 1.3em;
    font-weight: 700;
    line-height: 1;
    color: var(--txt2);
    border: 1px solid transparent;
    cursor: pointer;
  }
  .ic:hover:not([disabled]), .ic:focus-visible {
    color: var(--hi);
    border-color: rgba(var(--accent-rgb), 0.6);
    background: rgba(var(--accent-rgb), 0.15);
  }
  .ic[disabled] { opacity: 0.35; cursor: default; }

  .repo { border-bottom: 1px solid var(--line-soft); padding: 4px 0 8px; }
  .rhead { position: relative; display: flex; align-items: center; gap: 8px; padding: 3px 10px 3px 6px; font-size: 1.05em; }
  .caret { color: var(--dim); width: 1em; display: inline-block; }
  .rico { width: 14px; height: 14px; fill: none; stroke: var(--txt3); stroke-width: 1.3; vertical-align: -2px; margin-right: 4px; }
  .twist { all: unset; cursor: pointer; color: var(--txt2); white-space: nowrap; }
  .twist b { color: var(--hi); font-weight: 700; }
  .twist:focus-visible { outline: 1px solid var(--accent); }
  .branch { all: unset; color: var(--txt3); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0; cursor: pointer; padding: 0 3px; border: 1px solid transparent; }
  .branch:hover:not([disabled]), .branch:focus-visible, .branch[aria-expanded='true'] { color: var(--hi); border-color: rgba(var(--accent-rgb), 0.6); background: rgba(var(--accent-rgb), 0.12); }
  .branch[disabled] { opacity: 0.5; cursor: default; }
  .ab { all: unset; color: var(--txt2); cursor: pointer; white-space: nowrap; font-variant-numeric: tabular-nums; }
  .ab:hover:not([disabled]), .ab:focus-visible { color: var(--hi); }
  .ab[disabled] { opacity: 0.5; cursor: default; }
  .busy { color: var(--accent); animation: blink 1s ease-in-out infinite; }
  @keyframes blink { 50% { opacity: 0.45; } }
  .err { margin: 4px 8px; padding: 3px 6px; color: var(--red); background: rgba(var(--red-rgb), 0.1); border-left: 2px solid var(--red); word-break: break-word; }

  .msg { position: relative; margin: 6px 8px 0; }
  .ai {
    all: unset;
    position: absolute;
    top: 3px;
    right: 3px;
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    border-radius: 3px;
    color: var(--accent);
    font-size: 1.1em;
    cursor: pointer;
    opacity: 0.75;
  }
  .ai:hover:not([disabled]), .ai:focus-visible { opacity: 1; background: rgba(var(--accent-rgb), 0.18); }
  .ai[disabled] { opacity: 0.3; cursor: default; }
  .ai.writing { opacity: 1; color: var(--accent); animation: blink 1s ease-in-out infinite; }
  textarea {
    display: block;
    box-sizing: border-box;
    width: 100%;
    margin: 0;
    padding: 4px 26px 4px 6px;
    resize: vertical;
    background: rgba(var(--bg-rgb), 0.6);
    color: var(--txt);
    border: 1px solid var(--line);
    font: inherit;
  }
  textarea:focus { outline: none; border-color: var(--accent); }
  textarea::placeholder { color: var(--faint); }

  .actions { position: relative; display: flex; gap: 0; margin: 6px 8px 4px; }
  .actions > button { all: unset; padding: 2px 10px; border: 1px solid var(--line); color: var(--txt2); cursor: pointer; white-space: nowrap; }
  .actions > button:hover:not([disabled]), .actions > button:focus-visible { border-color: var(--accent); color: var(--hi); }
  .actions > button[disabled] { opacity: 0.4; cursor: default; }
  .actions .primary { flex: 1; padding: 4px 10px; text-align: center; background: var(--accent); border-color: var(--accent); color: var(--ink); font-weight: 700; }
  .actions .more { background: var(--accent); border-color: var(--accent); color: var(--ink); border-left: 1px solid rgba(var(--bg-rgb), 0.4); }
  .actions .primary[disabled] { opacity: 0.45; }
  .actions .primary:hover:not([disabled]) { color: var(--ink); background: var(--accent-soft); }
  .menu {
    position: absolute;
    top: calc(100% + 2px);
    left: 0;
    right: 0;
    z-index: 2;
    display: flex;
    flex-direction: column;
    background: var(--panel);
    border: 1px solid var(--accent);
  }
  .menu button { all: unset; padding: 4px 10px; color: var(--txt2); cursor: pointer; }
  .menu button:hover, .menu button:focus-visible { background: rgba(var(--accent-rgb), 0.15); color: var(--hi); }

  .lhead { display: flex; align-items: center; gap: 6px; padding: 8px 10px 3px; color: var(--dim); font-size: 0.85em; font-weight: 600; text-transform: uppercase; letter-spacing: 0.05em; }
  .lhead .count { padding: 0 6px; border-radius: 8px; background: var(--accent); color: var(--ink); font-weight: 700; }
  .hist { all: unset; display: flex; gap: 6px; align-items: center; padding: 8px 8px 2px; color: var(--dim); font-size: 0.85em; font-weight: 600; text-transform: uppercase; letter-spacing: 0.05em; cursor: pointer; }
  .hist .count { padding: 0 5px; background: var(--line-soft); color: var(--txt3); }
  .hist:hover { color: var(--hi); }
  ul, ol { list-style: none; margin: 0; padding: 0; }
  .file { display: flex; align-items: center; gap: 4px; padding: 0 10px 0 12px; height: 24px; font-size: 1.08em; }
  .file:hover { background: rgba(var(--accent-rgb), 0.07); }
  .code { width: 1.4ch; flex: none; font-weight: 700; text-align: right; }
  .cM { color: var(--amber); }
  .cA, .cQ { color: var(--green); }
  .cD { color: var(--red); }
  .cR, .cC, .cT { color: var(--blue); }
  .cU { color: var(--warn); }
  .fname { all: unset; flex: 1; min-width: 0; display: flex; gap: 6px; align-items: center; overflow: hidden; white-space: nowrap; cursor: pointer; }
  .fname:focus-visible { outline: 1px solid var(--accent); }
  .file:has(.fname.open) { background: rgba(var(--accent-rgb), 0.16); box-shadow: inset 0 0 0 1px rgba(var(--accent-rgb), 0.6); }
  .star { color: var(--violet); }
  .n { color: var(--txt); }
  .n.del { text-decoration: line-through; color: var(--dim); }
  .d { color: var(--faint); overflow: hidden; text-overflow: ellipsis; min-width: 0; font-size: 0.88em; }
  .fact { display: none; gap: 3px; }
  .file:hover .fact, .file:focus-within .fact { display: flex; }

  .graph li { display: flex; align-items: center; gap: 6px; padding: 0 8px 0 6px; white-space: nowrap; overflow: hidden; }
  .graph svg { flex: none; overflow: visible; }
  .graph line { stroke-width: 1.6; }
  .graph circle { stroke: var(--panel); stroke-width: 1.5; }
  .graph circle.head { stroke: var(--hi); }
  .l0, .d0 { stroke: var(--accent); fill: var(--accent); }
  .l1, .d1 { stroke: var(--cyan); fill: var(--cyan); }
  .l2, .d2 { stroke: var(--violet); fill: var(--violet); }
  .l3, .d3 { stroke: var(--green); fill: var(--green); }
  .l4, .d4 { stroke: var(--amber); fill: var(--amber); }
  .l5, .d5 { stroke: var(--blue); fill: var(--blue); }
  line.l0, line.l1, line.l2, line.l3, line.l4, line.l5 { fill: none; }
  .subj { color: var(--txt2); overflow: hidden; text-overflow: ellipsis; min-width: 0; flex: 1; }
  .ref { flex: none; padding: 0 4px; border: 1px solid var(--line); color: var(--txt3); font-size: 0.88em; }
  .ref.headref { border-color: var(--accent); color: var(--accent); }
  .ref.remote { color: var(--cyan); border-color: rgba(var(--cyan-rgb), 0.4); }
  .ref.tag { color: var(--amber); border-color: rgba(var(--amber-rgb), 0.4); }
  .when { flex: none; color: var(--faint); }
</style>
