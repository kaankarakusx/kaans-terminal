// Source Control durumu (OZELLIKLER.md §4): sohbetlerin ve terminallerin çalıştığı repolar, durumları, geçmişleri.
// Panel açıkken 4 sn'de bir, kapalıyken (rozet için) 20 sn'de bir yoklanır; uygulama arkadayken yoklanmaz.
import { invoke } from '@tauri-apps/api/core'
import { engine, inTauri } from './engine.svelte'
import { power } from './power.svelte'
import type { GitBranch, GitChange, GitCommit, RepoRef, RepoStatus } from './types'

const OPEN_POLL = 4_000
const IDLE_POLL = 20_000
const LOG_LIMIT = 120

export type GitOp = 'stage' | 'unstage' | 'discard' | 'commit' | 'push' | 'sync' | 'switch' | 'branch'

export interface CheckoutOpts {
  /** Yeni dal (`from` üzerinde; yoksa HEAD) */
  create?: boolean
  from?: string | null
  /** Önce değişiklikleri stash'e koy */
  stash?: boolean
}

// ---------------------------------------------------------------- arka uç (Tauri ya da demo)

interface Backend {
  repos(cwds: string[]): Promise<RepoRef[]>
  status(root: string): Promise<RepoStatus>
  log(root: string, limit: number): Promise<GitCommit[]>
  run(root: string, op: GitOp, paths: string[], message: string): Promise<void>
  branches(root: string): Promise<GitBranch[]>
  checkout(root: string, name: string, o: CheckoutOpts): Promise<void>
  edited(): Promise<string[]>
  /** Kullanıcının kendi Claude Code'u ile commit mesajı (`claude -p`) */
  aiMessage(root: string): Promise<string>
}

const tauri: Backend = {
  repos: (cwds) => invoke('git_repos', { cwds }),
  status: (root) => invoke('git_status', { root }),
  log: (root, limit) => invoke('git_log', { root, limit }),
  run: (root, op, paths, message) => invoke('git_run', { root, op, paths, message }),
  branches: (root) => invoke('git_branches', { root }),
  checkout: (root, name, o) =>
    invoke('git_checkout', { root, name, create: !!o.create, from: o.from ?? null, stash: !!o.stash }),
  edited: () => invoke('git_edited'),
  aiMessage: (root) => invoke('git_ai_message', { root }),
}

/** Tarayıcı demosu: tek sahte repo, işlemler bellekte. */
function demoBackend(): Backend {
  const root = '/Users/demo/work/api-server'
  const st: RepoStatus = {
    root, branch: 'feature/login', detached: false, upstream: 'origin/feature/login', ahead: 1, behind: 0,
    initial: false, truncated: false, conflicts: [],
    staged: [{ path: 'src/auth.rs', orig: null, code: 'M' }],
    changes: [
      { path: 'src/db.rs', orig: null, code: 'M' },
      { path: 'src/session/token.rs', orig: null, code: '?' },
      { path: 'README.md', orig: null, code: 'M' },
      { path: 'old/legacy.rs', orig: null, code: 'D' },
    ],
  }
  const t = Date.now()
  const log: GitCommit[] = [
    { hash: 'f1', parents: ['e1'], author: 'Kaan', time: t - 6e5, refs: ['HEAD -> feature/login'], subject: 'auth: refresh tokens' },
    { hash: 'e1', parents: ['d1', 'x1'], author: 'Kaan', time: t - 3.6e6, refs: ['origin/feature/login'], subject: 'merge main into feature/login' },
    { hash: 'x1', parents: ['c1'], author: 'Ada', time: t - 5e6, refs: ['origin/main', 'main'], subject: 'ci: cache cargo registry' },
    { hash: 'd1', parents: ['c1'], author: 'Kaan', time: t - 7e6, refs: [], subject: 'login form validation' },
    { hash: 'c1', parents: ['b1'], author: 'Kaan', time: t - 9e7, refs: ['tag: v0.3.0'], subject: 'release 0.3.0' },
    { hash: 'b1', parents: [], author: 'Kaan', time: t - 2e8, refs: [], subject: 'initial commit' },
  ]
  const move = (from: GitChange[], to: GitChange[], paths: string[], staging: boolean) => {
    for (const p of paths) {
      const i = from.findIndex((c) => c.path === p)
      if (i < 0) continue
      const [c] = from.splice(i, 1)
      to.push({ ...c, code: staging && c.code === '?' ? 'A' : !staging && c.code === 'A' ? '?' : c.code })
    }
  }
  const branches: GitBranch[] = [
    { name: 'feature/login', remote: false, current: true, upstream: 'origin/feature/login', time: t - 6e5, subject: 'auth: refresh tokens' },
    { name: 'main', remote: false, current: false, upstream: 'origin/main', time: t - 5e6, subject: 'ci: cache cargo registry' },
    { name: 'fix/typo', remote: false, current: false, upstream: null, time: t - 4e7, subject: 'docs: typo' },
    { name: 'origin/feature/login', remote: true, current: false, upstream: null, time: t - 3.6e6, subject: 'merge main into feature/login' },
    { name: 'origin/main', remote: true, current: false, upstream: null, time: t - 5e6, subject: 'ci: cache cargo registry' },
    { name: 'origin/release/0.4', remote: true, current: false, upstream: null, time: t - 8e7, subject: 'release: bump to 0.4' },
  ]
  return {
    branches: async () => structuredClone(branches),
    checkout: async (_r, name, o) => {
      await new Promise((r) => setTimeout(r, 250))
      if (!o.stash && !o.create && st.changes.length) throw 'Your changes would be overwritten by switching. Commit or stash them first.'
      if (o.stash) st.changes = []
      let local = name
      if (o.create) {
        if (branches.some((b) => !b.remote && b.name === name)) throw `A branch named ${name} already exists.`
        branches.unshift({ name, remote: false, current: false, upstream: null, time: Date.now(), subject: log[0].subject })
      } else if (branches.find((b) => b.name === name)?.remote) {
        local = name.slice(name.indexOf('/') + 1)
        if (!branches.some((b) => !b.remote && b.name === local))
          branches.unshift({ name: local, remote: false, current: false, upstream: name, time: Date.now(), subject: '' })
      }
      for (const b of branches) b.current = !b.remote && b.name === local
      st.branch = local
      st.upstream = branches.find((b) => b.name === local)?.upstream ?? null
    },
    repos: async () => [{ root, name: 'api-server' }],
    status: async () => structuredClone(st),
    log: async () => log,
    edited: async () => [`${root}/src/auth.rs`, `${root}/src/db.rs`],
    aiMessage: async () => {
      await new Promise((r) => setTimeout(r, 1200))
      return 'feat(auth): add token refresh to login flow\n\n- Retry the request once after refreshing an expired token\n- Show a clear error when refresh fails'
    },
    run: async (_r, op, paths, message) => {
      await new Promise((r) => setTimeout(r, 250))
      if (op === 'stage') move(st.changes, st.staged, paths, true)
      else if (op === 'unstage') move(st.staged, st.changes, paths, false)
      else if (op === 'discard') st.changes = st.changes.filter((c) => !paths.includes(c.path))
      else if (op === 'commit') {
        if (!message.trim()) throw 'commit message is empty'
        log.unshift({ hash: `n${log.length}`, parents: [log[0].hash], author: 'Kaan', time: Date.now(), refs: ['HEAD -> feature/login'], subject: message })
        log[1].refs = []
        st.staged = []
        st.ahead++
      } else if (op === 'push' || op === 'sync') st.ahead = 0
    },
  }
}

const backend: Backend = inTauri ? tauri : demoBackend()

// ---------------------------------------------------------------- durum

class Git {
  repos = $state.raw<RepoRef[]>([])
  status = $state.raw<Record<string, RepoStatus>>({})
  logs = $state.raw<Record<string, GitCommit[]>>({})
  branches = $state.raw<Record<string, GitBranch[]>>({})
  errors = $state<Record<string, string>>({})
  /** Repo başına çalışan işlem (düğmeler kilitlenir) */
  busy = $state<Record<string, GitOp | undefined>>({})
  /** Commit mesajı taslakları */
  messages = $state<Record<string, string>>({})
  /** AI commit mesajı yazılıyor (repo kökü) */
  writing = $state<Record<string, boolean>>({})
  edited = $state.raw<Set<string>>(new Set())
  /** Panel açık mı (yoklama sıklığı) */
  visible = $state(false)

  private reposKey = ''
  private timer: ReturnType<typeof setTimeout> | null = null
  private started = false

  /** Değişen dosya sayısı (rozet) */
  get count(): number {
    let n = 0
    for (const s of Object.values(this.status)) {
      n += new Set([...s.staged, ...s.changes, ...s.conflicts].map((c) => c.path)).size
    }
    return n
  }

  /** Sohbetlerin ve terminallerin klasörleri */
  private cwds(): string[] {
    const snap = engine.snapshot
    if (!snap) return []
    return [...snap.sessions.filter((s) => s.status !== 'ended').map((s) => s.cwd), ...snap.terminals.map((t) => t.cwd)]
      .filter(Boolean)
  }

  start() {
    if (this.started) return
    this.started = true
    void this.tick()
  }

  setVisible(v: boolean) {
    this.visible = v
    if (v) void this.refresh(true)
  }

  private schedule() {
    if (this.timer) clearTimeout(this.timer)
    this.timer = setTimeout(() => void this.tick(), this.visible ? OPEN_POLL : IDLE_POLL)
  }

  private async tick() {
    if (power.mode === 'foreground' || this.visible) await this.refresh(false)
    this.schedule()
  }

  /** Repoları bul, durumları (ve panel açıksa geçmişleri) yenile. */
  async refresh(withLog: boolean) {
    const cwds = this.cwds()
    const key = [...new Set(cwds)].sort().join('\n')
    if (key !== this.reposKey) {
      this.reposKey = key
      try {
        this.repos = await backend.repos(cwds)
      } catch {
        /* git yok ya da okunamadı: liste boş kalır */
      }
    }
    const roots = this.repos.map((r) => r.root)
    const next: Record<string, RepoStatus> = {}
    await Promise.all(
      roots.map(async (root) => {
        try {
          next[root] = await backend.status(root)
          if (this.errors[root]?.startsWith('status:')) delete this.errors[root]
        } catch (e) {
          this.errors[root] = `status: ${String(e)}`
        }
      }),
    )
    this.status = next
    if (this.visible) {
      try {
        this.edited = new Set(await backend.edited())
      } catch {
        /* ✦ yalnızca bir süs */
      }
      if (withLog) await Promise.all(roots.map((r) => this.loadLog(r)))
    }
  }

  async loadLog(root: string) {
    try {
      this.logs = { ...this.logs, [root]: await backend.log(root, LOG_LIMIT) }
    } catch (e) {
      this.errors[root] = String(e)
    }
  }

  /** İşlem; bitince durum ve geçmiş yenilenir. Hata repo başlığının altında görünür. */
  async run(root: string, op: GitOp, paths: string[] = [], message = ''): Promise<boolean> {
    if (this.busy[root]) return false
    this.busy[root] = op
    delete this.errors[root]
    try {
      await backend.run(root, op, paths, message)
      return true
    } catch (e) {
      this.errors[root] = String(e)
      return false
    } finally {
      this.busy[root] = undefined
      await this.refresh(false)
      if (op === 'commit' || op === 'push' || op === 'sync') await this.loadLog(root)
    }
  }

  async loadBranches(root: string) {
    try {
      this.branches = { ...this.branches, [root]: await backend.branches(root) }
    } catch (e) {
      this.errors[root] = String(e)
    }
  }

  /** Dala geç / dal oluştur. Hata metnini döndürür (seçici "stash & switch" önerebilsin); başarıda null. */
  async checkout(root: string, name: string, o: CheckoutOpts = {}): Promise<string | null> {
    if (this.busy[root]) return 'busy'
    this.busy[root] = o.create ? 'branch' : 'switch'
    delete this.errors[root]
    try {
      await backend.checkout(root, name, o)
      return null
    } catch (e) {
      return String(e)
    } finally {
      this.busy[root] = undefined
      await this.refresh(false)
      await Promise.all([this.loadLog(root), this.loadBranches(root)])
    }
  }

  /** ✨ Commit mesajını kullanıcının Claude Code'u yazar (staged varsa onlar, yoksa bütün değişiklikler). */
  async aiMessage(root: string) {
    if (this.writing[root]) return
    this.writing[root] = true
    delete this.errors[root]
    try {
      const msg = await backend.aiMessage(root)
      this.messages[root] = msg
    } catch (e) {
      this.errors[root] = String(e)
    } finally {
      this.writing[root] = false
    }
  }

  /** Commit (+ push / sync). Staged değişiklik yoksa `stageAll` ile önce hepsi eklenir. */
  async commit(root: string, then: 'none' | 'push' | 'sync', stageAll = false) {
    const message = this.messages[root] ?? ''
    const st = this.status[root]
    if (stageAll && st) {
      const paths = [...st.changes.map((c) => c.path)]
      if (paths.length && !(await this.run(root, 'stage', paths))) return
    }
    if (!(await this.run(root, 'commit', [], message))) return
    this.messages[root] = ''
    if (then !== 'none') await this.run(root, then)
  }

  isEdited(root: string, path: string): boolean {
    return this.edited.has(`${root}/${path}`)
  }
}

export const git = new Git()
