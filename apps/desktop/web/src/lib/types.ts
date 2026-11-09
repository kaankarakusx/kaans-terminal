// Rust tarafındaki serde tipleriyle birebir (src-tauri/src/engine/mod.rs).

export type Status = 'waiting' | 'idle' | 'busy' | 'ended'

export type Category =
  | 'web' | 'browser' | 'read' | 'search' | 'edit' | 'bash' | 'agent' | 'mcp' | 'plan' | 'other'

export interface Tokens {
  input: number
  output: number
  cacheRead: number
  cacheWrite: number
  cacheWrite1h: number
}

export interface SubagentView {
  id: string
  kind: string
  description: string
  running: boolean
  lastTool: string | null
  /** Sonucu beklenen araç (şu an ne yapıyor). */
  currentTool: string | null
  started: number
}

export interface WebItem {
  ts: number
  session: string
  kind: 'search' | 'fetch' | 'browser'
  target: string
  subagent: boolean
}

export interface FileHeat {
  path: string
  reads: number
  writes: number
  last: number
  sessions: string[]
  /** Son 20 dakikada iki farklı sohbet bu dosyaya yazdı. */
  clash: boolean
}

export interface Clash {
  path: string
  sessions: string[]
  last: number
}

export type LedgerKey = 'today' | '7d' | '30d' | '12w' | '12m'

export interface LedgerRow {
  project: string
  model: string
  tokens: number
  cost: number
}

export interface LedgerRange {
  key: LedgerKey
  cost: number
  tokens: number
  rows: LedgerRow[]
}

export interface LedgerView {
  /** Geçmiş günler henüz okunmadıysa false. */
  ready: boolean
  ranges: LedgerRange[]
  /** Son 30 günün günlük maliyeti; son eleman bugün. */
  daily: number[]
}

export interface PowerState {
  onBattery: boolean
  lowPower: boolean
}

export interface SessionView {
  id: string
  /** "claude" | "codex" | "gemini" | "antigravity" | … */
  tool: string
  pid: number | null
  title: string
  project: string
  cwd: string
  status: Status
  waitingFor: string | null
  model: string | null
  contextTokens: number
  /** Model penceresi ile /autocompact penceresinin küçüğü; model bilinmiyorsa null. */
  contextWindow: number | null
  contextPct: number | null
  tokens: Tokens
  /** Bugünkü api-eq maliyeti ($). */
  cost: number
  /** Fiyat tablosunda olmayan bir modelin token'ları var (maliyet eksik). */
  unpriced: boolean
  /** Son 30 dakikanın dakika başına araç çağrıları; son eleman şimdiki dakika. */
  activity: number[]
  /** Başka bir sohbetle son 20 dakikada aynı dosyaya yazılan yollar. */
  sameFile: string[]
  /** Bu sohbet uygulamanın bir terminalinde çalışıyorsa o terminal. */
  terminal: string | null
  tools: Partial<Record<Category, number>>
  toolCalls: number
  toolErrors: number
  webCalls: number
  prompts: number
  filesEdited: number
  filesRead: number
  lastPrompt: string | null
  lastTool: string | null
  lastActivity: number
  hue: number
  /** Effort seviyesi (Claude) */
  effort: string | null
  /** Son prompt'un zamanı (çalışırken turun süresi buradan) */
  turnStarted: number
  subagents: SubagentView[]
}

export interface Kpis {
  sessionsLive: number
  agentsRunning: number
  toolsPerMin: number
  tokensPerMin: number
  webCalls: number
  toolCalls: number
  toolErrors: number
  toolOkPct: number | null
  cacheHitPct: number | null
  tokensToday: Tokens
  costToday: number
  burnPerHour: number
  cacheSavingsToday: number
  /** Bugünkü araç çağrıları kategoriye göre. */
  toolMix: Partial<Record<Category, number>>
}

export interface Snapshot {
  generatedAt: number
  day: string
  sessions: SessionView[]
  kpis: Kpis
  web: WebItem[]
  files: FileHeat[]
  clashes: Clash[]
  ledger: LedgerView
  terminals: TerminalView[]
  limits: ToolLimits[]
  /** Çalışan uygulama komutları (dev sunucuları) */
  apps: AppView[]
  leftovers: Leftover[]
  runaways: Runaway[]
}

export interface LimitWindow {
  usedPct: number
  windowMinutes: number
  /** Unix saniyesi */
  resetsAt: number
}

export interface ToolLimits {
  tool: string
  fiveHour: LimitWindow | null
  weekly: LimitWindow | null
  updated: number
}

/** Çalışan uygulama komutu (dev sunucusu vb.): orbit.map'te araba */
export interface AppView {
  pid: number
  label: string
  project: string
  cwd: string
  cpuPct: number
  started: number
  /** Uygulamanın bir terminalinde çalışıyorsa o terminal */
  terminal: string | null
}

export interface TerminalView {
  id: string
  cwd: string
  project: string
  /** Ön plandaki komut (boştaysa kabuk) */
  title: string
  busy: boolean
  /** İçinde Claude çalışıyorsa sohbet kartı */
  chat: string | null
  started: number
  attached: boolean
}

/** Kapanan terminalden kalan süreç (port tutan dev server gibi): amber araba. */
export interface Leftover {
  pid: number
  name: string
  cwd: string
  session: string
  since: number
}

/** Kaçak süreç (DANGER). */
export interface Runaway {
  pid: number
  name: string
  path: string | null
  cpuPct: number
  since: number
}

export type EventKind = 'prompt' | 'tool' | 'toolError' | 'reply'

export interface EngineEvent {
  ts: number
  session: string
  kind: EventKind
  tool: string | null
  category: Category | null
  target: string | null
  id: string | null
  subagent: boolean
}

export const STATUS_LABEL: Record<Status, string> = {
  busy: 'working',
  idle: 'your turn',
  waiting: 'needs you',
  ended: 'ended',
}

export const CATEGORIES: Category[] = [
  'web', 'browser', 'read', 'search', 'edit', 'bash', 'agent', 'mcp', 'plan', 'other',
]

export const STATUS_COLOR: Record<Status, string> = {
  busy: 'var(--accent)',
  idle: 'var(--cyan)',
  waiting: 'var(--warn)',
  ended: 'var(--ended)',
}

// ---- Source Control (src-tauri/src/git.rs)
export interface RepoRef {
  root: string
  name: string
}

export interface GitChange {
  /** Repo köküne göre yol */
  path: string
  orig: string | null
  /** M A D R C T U ? */
  code: string
}

export interface RepoStatus {
  root: string
  branch: string
  detached: boolean
  upstream: string | null
  ahead: number
  behind: number
  initial: boolean
  staged: GitChange[]
  changes: GitChange[]
  conflicts: GitChange[]
  truncated: boolean
}

export interface GitBranch {
  /** Kısa ad: `main`, uzak dalda `origin/main` */
  name: string
  remote: boolean
  current: boolean
  upstream: string | null
  /** Son commit (ms) */
  time: number
  subject: string
}

export interface GitCommit {
  hash: string
  parents: string[]
  author: string
  time: number
  refs: string[]
  subject: string
}
