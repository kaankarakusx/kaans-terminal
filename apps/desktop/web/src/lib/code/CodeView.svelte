<!--
  Dosya / diff görüntüleyici (OZELLIKLER.md §4, VS Code görünümü): sözdizimi renklendirme (temada), kelime kaydırma,
  sticky scroll, değişikliklerin overview ruler'ı, renk örnekleri, tıklanan satırın blame'i, durum çubuğu.
  Source Control'de bir dosyaya tıklayınca orta alanda açılır; Esc kapatır.
-->
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { untrack } from 'svelte'
  import { inTauri } from '../engine.svelte'
  import { ago } from '../format'
  import { git } from '../git.svelte'
  import { now } from '../now.svelte'
  import { ui, type CodeTarget } from '../ui.svelte'
  import { COLOR_RE, allAdded, allDeleted, lineCount, parsePatch, stickyLines, type DiffLine } from './diff'
  import FileIcon from './FileIcon.svelte'
  import { langLabel, langOf, tokenize, type Tok } from './highlight'

  interface FileText { text: string; missing: boolean; binary: boolean; tooLarge: boolean }
  interface DiffView { old: FileText; new: FileText; patch: string }
  interface Blame { hash: string; author: string; time: number; summary: string; uncommitted: boolean }

  let { target }: { target: CodeTarget } = $props()

  const ROW = 20
  /** Kaydırma açıkken sanal liste kapanır; bu kadar satırın üstünde kaydırma sunulmaz */
  const WRAP_MAX = 4000

  let mode = $state<'diff' | 'file'>('diff')
  let wrap = $state(false)
  let data = $state<DiffView | null>(null)
  let error = $state<string | null>(null)
  let oldToks = $state.raw<Tok[][]>([])
  let newToks = $state.raw<Tok[][]>([])
  let selected = $state<number | null>(null)
  let blame = $state<Blame | null>(null)
  let scrollTop = $state(0)
  let viewH = $state(400)
  let scroller = $state<HTMLElement>()

  const lang = $derived(langOf(target.path))

  // Yükle (hedef değişince baştan)
  $effect(() => {
    const t = target
    mode = t.mode
    data = null
    error = null
    selected = null
    blame = null
    scrollTop = 0
    // Yalnızca hedefe bağlı kalsın: kaydırma alanı (bind:this) her yüklemede yeniden oluşur.
    untrack(() => {
      if (scroller) scroller.scrollTop = 0
    })
    let alive = true
    void load(t).then(
      (d) => {
        if (!alive) return
        data = d
        void tokenize(d.old.text, langOf(t.orig ?? t.path)).then((x) => alive && (oldToks = x))
        void tokenize(d.new.text, langOf(t.path)).then((x) => alive && (newToks = x))
      },
      (e) => alive && (error = String(e)),
    )
    return () => {
      alive = false
    }
  })

  async function load(t: CodeTarget): Promise<DiffView> {
    if (!inTauri) return demoDiff(t)
    return invoke<DiffView>('git_diff', { root: t.root, path: t.path, orig: t.orig, staged: t.staged })
  }

  const patchLines = $derived(data ? parsePatch(data.patch) : [])
  /** Diff satırları: git'in patch'i; yoksa (izlenmeyen/silinen) hepsi eklenmiş/silinmiş */
  const diffRows: DiffLine[] = $derived.by(() => {
    if (!data) return []
    if (patchLines.length) return patchLines
    if (data.old.missing && !data.new.missing) return allAdded(lineCount(data.new.text))
    if (data.new.missing && !data.old.missing) return allDeleted(lineCount(data.old.text))
    return []
  })
  const newLines = $derived(data ? data.new.text.split('\n') : [])
  const fileRows: DiffLine[] = $derived(
    Array.from({ length: data ? lineCount(data.new.text) : 0 }, (_, i) => ({ kind: 'ctx' as const, old: null, new: i + 1 })),
  )
  const rows = $derived(mode === 'diff' ? diffRows : fileRows)

  /** Dosya görünümünde gutter ve ruler işaretleri: yeni dosyada eklenen satırlar, silinmenin olduğu yer */
  const marks = $derived.by(() => {
    const added = new Set<number>()
    const deletedAt = new Set<number>()
    let lastNew = 0
    for (const l of diffRows) {
      if (l.kind === 'add' && l.new) added.add(l.new)
      if (l.new) lastNew = l.new
      if (l.kind === 'del') deletedAt.add(lastNew + 1)
    }
    return { added, deletedAt }
  })

  const canWrap = $derived(rows.length <= WRAP_MAX)
  const virtual = $derived(!(wrap && canWrap))
  const first = $derived(virtual ? Math.max(0, Math.floor(scrollTop / ROW) - 30) : 0)
  const last = $derived(virtual ? Math.min(rows.length, Math.ceil((scrollTop + viewH) / ROW) + 30) : rows.length)
  const visible = $derived(rows.slice(first, last).map((r, i) => ({ r, i: first + i })))

  // Sticky scroll: yalnızca dosya görünümünde ve sanal listede (satır yüksekliği sabit)
  const sticky = $derived(mode === 'file' && virtual ? stickyLines(newLines, Math.floor(scrollTop / ROW)) : [])

  function toksOf(r: DiffLine): Tok[] {
    if (r.kind === 'del' && r.old) return oldToks[r.old - 1] ?? [{ content: data?.old.text.split('\n')[r.old - 1] ?? '' }]
    if (r.new) return newToks[r.new - 1] ?? [{ content: newLines[r.new - 1] ?? '' }]
    return []
  }

  /** Token'ı renk örneklerine böl */
  function pieces(t: Tok): { text: string; swatch?: string }[] {
    if (!t.content.includes('#')) return [{ text: t.content }]
    const out: { text: string; swatch?: string }[] = []
    let last = 0
    for (const m of t.content.matchAll(COLOR_RE)) {
      if (m.index! > last) out.push({ text: t.content.slice(last, m.index) })
      out.push({ text: m[0], swatch: m[0] })
      last = m.index! + m[0].length
    }
    if (last < t.content.length) out.push({ text: t.content.slice(last) })
    return out
  }

  async function pick(r: DiffLine) {
    const line = r.new
    if (!line || r.kind === 'hunk') return
    selected = line
    blame = null
    if (!inTauri) {
      blame = { hash: 'f1', author: 'Kaan', time: Date.now() - 3.6e6, summary: 'auth: refresh tokens', uncommitted: r.kind === 'add' }
      return
    }
    try {
      const b = await invoke<Blame | null>('git_blame', { root: target.root, path: target.path, line })
      if (selected === line) blame = b
    } catch {
      /* blame yoksa sessiz */
    }
  }

  const blameText = $derived(
    blame ? (blame.uncommitted ? 'You · uncommitted changes' : `${blame.author}, ${ago(now.value - blame.time)} ago · ${blame.summary}`) : '',
  )

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && !e.defaultPrevented) {
      e.preventDefault()
      ui.code = null
    }
  }

  const rulerMarks = $derived.by(() => {
    const total = Math.max(1, rows.length)
    const out: { top: number; kind: string }[] = []
    rows.forEach((r, i) => {
      if (mode === 'diff' ? r.kind === 'add' || r.kind === 'del' : r.new && (marks.added.has(r.new) || marks.deletedAt.has(r.new)))
        out.push({ top: (i / total) * 100, kind: r.kind === 'del' || (r.new != null && marks.deletedAt.has(r.new) && !marks.added.has(r.new)) ? 'del' : 'add' })
    })
    return out
  })

  const name = $derived(target.path.split('/').at(-1) ?? target.path)
  const crumbs = $derived(target.path.split('/'))
  const repoName = $derived(target.root.split('/').filter(Boolean).at(-1) ?? '')
  const repoStatus = $derived(git.status[target.root])
  const branch = $derived(repoStatus?.branch ?? '')
  const dirty = $derived(!!repoStatus && repoStatus.changes.length + repoStatus.staged.length > 0)
  /** Dosyanın git durumu (sekmede "(Untracked) U" gibi) */
  const fileState = $derived.by(() => {
    const list = target.staged ? repoStatus?.staged : repoStatus?.changes
    const c = list?.find((x) => x.path === target.path)
    if (!c) return null
    const LABEL: Record<string, string> = { M: 'Modified', A: 'Added', D: 'Deleted', R: 'Renamed', C: 'Copied', T: 'Type changed', U: 'Conflict', '?': 'Untracked' }
    return { code: c.code === '?' ? 'Q' : c.code, letter: c.code === '?' ? 'U' : c.code, label: LABEL[c.code] ?? c.code }
  })
  const eol = $derived(data?.new.text.includes('\r\n') ? 'CRLF' : 'LF')
  /** Girinti: sekme mi, kaç boşluk mu (ilk girintili satırlardan) */
  const indentLabel = $derived.by(() => {
    let min = 0
    for (const l of newLines.slice(0, 400)) {
      if (l.startsWith('\t')) return 'Tab Size: 4'
      const m = l.match(/^ +(?=\S)/)
      if (m && (min === 0 || m[0].length < min)) min = m[0].length
    }
    return `Spaces: ${min || 2}`
  })
  const LANG_NAMES: Record<string, string> = {
    typescript: 'TypeScript', tsx: 'TypeScript JSX', javascript: 'JavaScript', jsx: 'JavaScript JSX', json: 'JSON',
    rust: 'Rust', svelte: 'Svelte', css: 'CSS', html: 'HTML', markdown: 'Markdown', python: 'Python', go: 'Go',
    toml: 'TOML', yaml: 'YAML', shellscript: 'Shell Script', sql: 'SQL', java: 'Java', kotlin: 'Kotlin',
    swift: 'Swift', c: 'C', cpp: 'C++',
  }
  const prettyLang = (l: string | null) => (l ? (LANG_NAMES[l] ?? l) : langLabel(null))
  const unavailable = $derived(
    data && (mode === 'file' ? data.new : data.new.missing ? data.old : data.new),
  )

  function demoDiff(t: CodeTarget): DiffView {
    const old = `// ${t.path}\nimport { db } from './db'\n\nexport async function login(user: string) {\n  const row = await db.find(user)\n  return row?.token\n}\n`
    const neu = `// ${t.path}\nimport { db } from './db'\nimport { refresh } from './session/token'\n\nconst ACCENT = '#f88008'\n\nexport async function login(user: string) {\n  const row = await db.find(user)\n  if (!row) throw new Error('no such user')\n  return refresh(row.token)\n}\n`
    const patch = `@@ -1,7 +1,11 @@\n // ${t.path}\n import { db } from './db'\n+import { refresh } from './session/token'\n \n+const ACCENT = '#f88008'\n+\n export async function login(user: string) {\n   const row = await db.find(user)\n-  return row?.token\n+  if (!row) throw new Error('no such user')\n+  return refresh(row.token)\n }\n`
    const f = (text: string) => ({ text, missing: false, binary: false, tooLarge: false })
    return { old: f(old), new: f(neu), patch }
  }
</script>

<svelte:window {onkeydown} />

<section class="code" aria-label="{mode} · {target.path}">
  <!-- Sekme (VS Code gibi): simge, ad, durum, kapat · sağda görünüm düğmeleri -->
  <header>
    <span class="tab" title={target.path}>
      <FileIcon path={target.path} />
      <b>{name}</b>
      {#if fileState}<span class="state c{fileState.code}">({fileState.label})</span>{/if}
      {#if git.isEdited(target.root, target.path)}<span class="star" title="edited by a chat today">✦</span>{/if}
      {#if fileState}<span class="letter c{fileState.code}">{fileState.letter}</span>{/if}
      <button class="x" title="close (Esc)" aria-label="close" onclick={() => (ui.code = null)}>×</button>
    </span>
    <span class="grow"></span>
    {#if mode === 'diff'}<span class="chip">{target.staged ? 'staged ↔ HEAD' : 'working tree ↔ staged'}</span>{/if}
    <span class="seg" role="group" aria-label="view">
      <button class:on={mode === 'diff'} aria-pressed={mode === 'diff'} onclick={() => (mode = 'diff')}>diff</button>
      <button class:on={mode === 'file'} aria-pressed={mode === 'file'} onclick={() => (mode = 'file')}>file</button>
    </span>
    <button class="tg" class:on={wrap} aria-pressed={wrap} disabled={!canWrap} title={canWrap ? 'word wrap' : 'too many lines to wrap'} onclick={() => (wrap = !wrap)}>wrap</button>
  </header>
  <nav class="crumbs" aria-label="path">
    <span class="repo">{repoName}</span>
    {#each crumbs as c, k (k)}<span class="sep">›</span><span class:last={k === crumbs.length - 1}>{#if k === crumbs.length - 1}<FileIcon path={target.path} />{/if}{c}</span>{/each}
  </nav>

  <div class="main">
    {#if error}
      <p class="msg err">✕ {error}</p>
    {:else if !data}
      <p class="msg">loading…</p>
    {:else if unavailable && (unavailable.binary || unavailable.tooLarge)}
      <p class="msg">{unavailable.binary ? 'Binary file — not shown.' : 'File is larger than 1 MB — not shown.'}</p>
    {:else if mode === 'file' && data.new.missing}
      <p class="msg">This file was deleted.</p>
    {:else if rows.length === 0}
      <p class="msg">{mode === 'diff' ? 'No changes.' : 'Empty file.'}</p>
    {:else}
      <div
        class="scroll"
        class:wrap={!virtual}
        bind:this={scroller}
        bind:clientHeight={viewH}
        onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}
      >
        {#if sticky.length}
          <div class="sticky" aria-hidden="true">
            {#each sticky as n (n)}
              <div class="row" style:height="{ROW}px">
                <span class="no">{n + 1}</span><span class="gm"></span>
                <span class="txt">{#each newToks[n] ?? [{ content: newLines[n] }] as t, k (k)}<span style:color={t.color} class:i={t.italic} class:b={t.bold}>{t.content}</span>{/each}</span>
              </div>
            {/each}
          </div>
        {/if}
        <div class="lines" style:height={virtual ? `${rows.length * ROW}px` : undefined}>
          {#each visible as { r, i } (i)}
            {@const isSel = r.new != null && r.new === selected}
            <div
              class="row {r.kind}"
              class:sel={isSel}
              class:fadd={mode === 'file' && r.new != null && marks.added.has(r.new)}
              class:fdel={mode === 'file' && r.new != null && marks.deletedAt.has(r.new)}
              style:top={virtual ? `${i * ROW}px` : undefined}
              style:min-height="{ROW}px"
              role="presentation"
              onclick={() => pick(r)}
            >
              {#if r.kind === 'hunk'}
                <span class="no"></span>{#if mode === 'diff'}<span class="no"></span>{/if}<span class="gm"></span>
                <span class="txt hunktxt">@@ {r.text}</span>
              {:else}
                {#if mode === 'diff'}<span class="no">{r.old ?? ''}</span>{/if}
                <span class="no">{r.new ?? ''}</span>
                <span class="gm">{r.kind === 'add' ? '+' : r.kind === 'del' ? '−' : ''}</span>
                <span class="txt"
                  >{#each toksOf(r) as t, k (k)}{#each pieces(t) as p, j (j)}{#if p.swatch}<i class="swatch" style:background={p.swatch}></i>{/if}<span style:color={t.color} class:i={t.italic} class:b={t.bold}>{p.text}</span>{/each}{/each}{#if isSel && blameText}<span class="blame">{blameText}</span>{/if}</span
                >
              {/if}
            </div>
          {/each}
        </div>
      </div>
      <div class="ruler" aria-hidden="true">
        {#each rulerMarks as m, k (k)}<i class={m.kind} style:top="{m.top}%"></i>{/each}
      </div>
    {/if}
  </div>

  <footer>
    {#if branch}<span class="br">⑂ {branch}{dirty ? '*' : ''}</span>{/if}
    {#if mode === 'diff'}<span class="add">+{diffRows.filter((l) => l.kind === 'add').length}</span><span class="del">−{diffRows.filter((l) => l.kind === 'del').length}</span>{/if}
    {#if blameText}<span class="bl" title={blame?.hash}>⎇ {blameText}</span>{/if}
    <span class="grow"></span>
    <span>Ln {selected ?? 1}, Col 1</span>
    <span>{indentLabel}</span>
    <span>UTF-8</span>
    <span>{eol}</span>
    <span class="lang">{'{}'} {prettyLang(lang)}</span>
  </footer>
</section>

<style>
  .code {
    height: 100%;
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--term-bg);
    border: 1px solid var(--line-soft);
    /* Sözdizimi renkleri temadan (highlight.ts → --shk-*) */
    --shk-foreground: var(--txt);
    --shk-background: transparent;
    --shk-token-keyword: var(--ansi-13);
    --shk-token-string: var(--ansi-10);
    --shk-token-string-expression: var(--ansi-14);
    --shk-token-constant: var(--ansi-11);
    --shk-token-function: var(--ansi-12);
    --shk-token-parameter: var(--txt2);
    --shk-token-punctuation: var(--txt3);
    --shk-token-comment: var(--dim);
    --shk-token-link: var(--cyan);
    --shk-token-inserted: var(--green);
    --shk-token-deleted: var(--red);
    --shk-token-changed: var(--amber);
  }
  header { display: flex; align-items: stretch; gap: 8px; height: 30px; padding-right: 8px; border-bottom: 1px solid var(--line-soft); flex: none; background: rgba(var(--panel-rgb), 0.9); }
  header > :not(.tab) { align-self: center; }
  .tab {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 8px 0 10px;
    min-width: 0;
    max-width: 60%;
    background: var(--term-bg);
    border-top: 2px solid var(--accent);
    border-right: 1px solid var(--line-soft);
    margin-bottom: -1px;
    font-size: 1.05em;
    white-space: nowrap;
  }
  .tab b { color: var(--hi); font-weight: 600; overflow: hidden; text-overflow: ellipsis; }
  .state { color: var(--dim); font-size: 0.92em; }
  .letter { font-weight: 700; }
  .cM { color: var(--amber); }
  .cA, .cQ { color: var(--green); }
  .cD { color: var(--red); }
  .cR, .cC, .cT { color: var(--blue); }
  .cU { color: var(--warn); }
  .state.cM, .state.cA, .state.cQ, .state.cD, .state.cR { opacity: 0.85; }
  .x { all: unset; padding: 0 2px 0 4px; color: var(--dim); cursor: pointer; }
  .x:hover, .x:focus-visible { color: var(--hi); }
  .star { color: var(--violet); }
  .chip { padding: 0 6px; border: 1px solid var(--line); color: var(--txt3); font-size: 0.85em; white-space: nowrap; }
  .grow { flex: 1; }
  .seg { display: flex; }
  .seg button, .tg { all: unset; padding: 0 8px; border: 1px solid var(--line); color: var(--dim); cursor: pointer; }
  .seg button + button { border-left: none; }
  .seg button.on, .tg.on { color: var(--ink); background: var(--accent); border-color: var(--accent); }
  .seg button:focus-visible, .tg:focus-visible { outline: 1px solid var(--hi); }
  .tg[disabled] { opacity: 0.35; cursor: default; }
  .crumbs { flex: none; display: flex; align-items: center; gap: 5px; height: 22px; padding: 0 12px; color: var(--dim); white-space: nowrap; overflow: hidden; border-bottom: 1px solid var(--line-soft); }
  .crumbs .sep { color: var(--faint); }
  .crumbs .last { display: inline-flex; align-items: center; color: var(--txt2); }
  .crumbs .repo { color: var(--txt3); }

  .main { position: relative; flex: 1; min-height: 0; display: flex; }
  .msg { color: var(--dim); margin: 16px; }
  .msg.err { color: var(--red); }
  .scroll { position: relative; flex: 1; overflow: auto; font-family: var(--font-mono); font-size: 1.12em; line-height: 20px; }
  .lines { position: relative; min-width: max-content; }
  .scroll.wrap .lines { min-width: 0; }
  .row { display: flex; align-items: flex-start; white-space: pre; cursor: text; }
  .lines .row { position: absolute; left: 0; right: 0; }
  .scroll.wrap .lines .row { position: static; }
  .scroll.wrap .txt { white-space: pre-wrap; word-break: break-all; }
  .no { flex: none; width: 4.2ch; padding-right: 1ch; text-align: right; color: var(--faint); user-select: none; position: sticky; left: 0; background: rgba(var(--term-bg-rgb), 1); }
  .no + .no { left: 5.2ch; }
  .gm { flex: none; align-self: stretch; width: 2ch; text-align: center; color: var(--faint); user-select: none; }
  .txt { flex: 1; padding-right: 2ch; color: var(--shk-foreground); }
  .i { font-style: italic; }
  .b { font-weight: 700; }
  .row.add { background: rgba(var(--green-rgb), 0.12); }
  .row.add .gm { color: var(--green); }
  .row.del { background: rgba(var(--red-rgb), 0.12); }
  .row.del .gm { color: var(--red); }
  .row.hunk { background: rgba(var(--accent-rgb), 0.07); color: var(--dim); }
  .hunktxt { color: var(--accent-soft); }
  /* Dosya görünümü: değişen satırın kenarında VS Code gibi şerit; silinen yerde kırmızı çentik */
  .row.fadd .gm { background: linear-gradient(90deg, transparent 30%, var(--green) 30%, var(--green) 60%, transparent 60%); }
  .row.fdel .gm { box-shadow: inset 0 3px 0 -1px var(--red); }
  .row:hover { background-color: rgba(var(--accent-rgb), 0.05); }
  .row.sel { background-color: rgba(var(--accent-rgb), 0.12); }
  .blame { margin-left: 4ch; color: var(--faint); font-style: italic; }
  .swatch { display: inline-block; width: 0.8em; height: 0.8em; margin-right: 0.3em; vertical-align: -0.05em; border: 1px solid var(--line); }
  .sticky {
    position: sticky;
    top: 0;
    z-index: 3;
    background: rgba(var(--panel-rgb), 0.98);
    border-bottom: 1px solid var(--line);
    box-shadow: 0 4px 10px rgba(0, 0, 0, 0.35);
  }
  .sticky .row { position: static; }

  .ruler { position: relative; flex: none; width: 10px; border-left: 1px solid var(--line-soft); }
  .ruler i { position: absolute; left: 1px; right: 1px; height: 3px; }
  .ruler i.add { background: var(--green); }
  .ruler i.del { background: var(--red); }

  footer { flex: none; display: flex; gap: 16px; align-items: center; height: 22px; padding: 0 10px; border-top: 1px solid var(--line-soft); color: var(--dim); background: rgba(var(--panel-rgb), 0.9); white-space: nowrap; overflow: hidden; }
  footer .add { color: var(--green); }
  footer .br { color: var(--txt2); }
  footer .lang { color: var(--txt2); }
  footer .del { color: var(--red); }
  .bl { color: var(--txt3); overflow: hidden; text-overflow: ellipsis; min-width: 0; }
</style>
