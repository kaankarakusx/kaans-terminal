// Yalnızca geliştirme: Tauri olmadan tarayıcıda arayüzü görmek için sahte engine.
// `engine.svelte.ts` bunu `import.meta.env.DEV` altında dinamik yükler; üretim paketine girmez.
import type {
  Category,
  EngineEvent,
  FileHeat,
  LedgerRange,
  SessionView,
  Snapshot,
  Status,
  WebItem,
} from './types'

const CATS: Category[] = ['read', 'edit', 'bash', 'search', 'web', 'agent', 'mcp', 'plan', 'browser']
const TOOLS: Record<Category, string> = {
  read: 'Read', edit: 'Edit', bash: 'Bash', search: 'Grep', web: 'WebFetch', agent: 'Agent',
  mcp: 'mcp__github__get_issue', plan: 'TodoWrite', browser: 'mcp__claude-in-chrome__navigate', other: 'Other',
}

let seed = 7
const rnd = () => ((seed = (seed * 16807) % 2147483647) - 1) / 2147483646
const pick = <T>(a: T[]) => a[Math.floor(rnd() * a.length)]

interface Demo {
  id: string
  title: string
  project: string
  status: Status
  model: string
  ctx: number
  subs: number
  waitingFor?: string
}

const BASE: Demo[] = [
  { id: 'd1', title: 'Fix login flow', project: 'api-server', status: 'busy', model: 'claude-opus-5-5', ctx: 412_000, subs: 2 },
  { id: 'd2', title: 'Refactor orbit camera', project: 'kaans-terminal', status: 'waiting', model: 'claude-opus-5-5', ctx: 188_000, subs: 0, waitingFor: 'permission' },
  { id: 'd3', title: 'Write migration', project: 'api-server', status: 'idle', model: 'claude-sonnet-5-5', ctx: 96_000, subs: 0 },
  { id: 'd4', title: 'Landing page copy', project: 'site', status: 'busy', model: 'claude-fable-5-1', ctx: 640_000, subs: 1 },
  { id: 'd5', title: 'Bench parser', project: 'parser', status: 'busy', model: 'claude-haiku-5-5', ctx: 31_000, subs: 0 },
  { id: 'd6', title: 'Old spike', project: 'parser', status: 'ended', model: 'claude-opus-5-5', ctx: 52_000, subs: 0 },
]

const FILES = [
  '~/work/api-server/src/auth.rs', '~/work/api-server/src/db.rs', '~/work/kaans-terminal/web/src/orbit/scene.ts',
  '~/work/site/index.html', '~/work/parser/src/lex.rs', '~/work/api-server/Cargo.toml',
]

export function startDemo(onSnapshot: (s: Snapshot) => void, onEvents: (e: EngineEvent[]) => void) {
  const start = Date.now()
  const tools = new Map<string, Partial<Record<Category, number>>>()
  const calls = new Map<string, number>()
  const activity = new Map<string, number[]>(BASE.map((d) => [d.id, Array.from({ length: 30 }, () => 0)]))
  const web: WebItem[] = []
  const heat = new Map<string, FileHeat>()
  let cost = 18.4
  let n = 0

  const tick = () => {
    const now = Date.now()
    const batch: EngineEvent[] = []
    for (const d of BASE) {
      if (d.status !== 'busy' || rnd() > 0.55) continue
      const cat = pick(CATS)
      const id = `t${n++}`
      const t = (tools.get(d.id) ?? {}) as Partial<Record<Category, number>>
      t[cat] = (t[cat] ?? 0) + 1
      tools.set(d.id, t)
      calls.set(d.id, (calls.get(d.id) ?? 0) + 1)
      activity.get(d.id)![29]++
      const target = cat === 'web' ? 'https://v2.tauri.app/reference' : cat === 'bash' ? 'cargo test --release' : pick(FILES)
      batch.push({ ts: now, session: d.id, kind: 'tool', tool: TOOLS[cat], category: cat, target, id, subagent: d.subs > 0 && rnd() < 0.3 })
      if (rnd() < 0.08) batch.push({ ts: now, session: d.id, kind: 'toolError', tool: TOOLS[cat], category: cat, target: null, id, subagent: false })
      if (cat === 'web') web.unshift({ ts: now, session: d.id, kind: rnd() < 0.5 ? 'search' : 'fetch', target: rnd() < 0.5 ? 'tauri window always on top' : 'https://threejs.org/docs/', subagent: false })
      if (cat === 'edit' || cat === 'read') {
        const p = target
        const f = heat.get(p) ?? { path: p, reads: 0, writes: 0, last: now, sessions: [], clash: false }
        if (cat === 'edit') f.writes++
        else f.reads++
        f.last = now
        if (!f.sessions.includes(d.id)) f.sessions.push(d.id)
        heat.set(p, f)
      }
      cost += 0.004 + rnd() * 0.02
    }
    if (rnd() < 0.15) batch.push({ ts: now, session: 'd1', kind: 'reply', tool: null, category: null, target: 'Tests pass; moving on to the token refresh path.', id: null, subagent: false })
    web.splice(30)
    // d1 ve d4 aynı dosyaya yazıyor: same file
    const clashPath = FILES[0]
    const cf = heat.get(clashPath) ?? { path: clashPath, reads: 2, writes: 3, last: now, sessions: ['d1', 'd4'], clash: true }
    cf.clash = true
    cf.sessions = [...new Set([...cf.sessions, 'd1', 'd4'])]
    heat.set(clashPath, cf)

    const sessions: SessionView[] = BASE.map((d, i) => {
      const t = tools.get(d.id) ?? {}
      return {
        id: d.id, tool: i === 4 ? 'codex' : 'claude', pid: d.status === 'ended' ? null : 1000 + i, title: d.title, project: d.project,
        cwd: `/Users/demo/work/${d.project}`, status: d.status, waitingFor: d.waitingFor ?? null, model: d.model,
        contextTokens: d.ctx, contextWindow: 1_000_000, contextPct: d.ctx / 10_000,
        tokens: { input: 1000, output: 40_000, cacheRead: 2_000_000, cacheWrite: 90_000, cacheWrite1h: 0 },
        cost: 1 + i * 2.3, unpriced: false, activity: activity.get(d.id)!, sameFile: d.id === 'd1' || d.id === 'd4' ? [clashPath] : [],
        tools: t, toolCalls: calls.get(d.id) ?? 0, toolErrors: i === 0 ? 2 : 0, webCalls: t.web ?? 0, prompts: 3 + i,
        terminal: d.id === 'd1' ? 'demo-1' : null,
        filesEdited: 2, filesRead: 9, lastPrompt: 'make the hexagons glow a bit more and park waiting sessions',
        lastTool: 'Edit', lastActivity: d.status === 'ended' ? start - 3_600_000 : now - (d.status === 'busy' ? 2000 : 95_000),
        hue: i, effort: i === 0 ? 'max' : 'high', turnStarted: start - 23 * 60_000, subagents: Array.from({ length: d.subs }, (_, k) => ({
          id: `${d.id}-a${k}`, kind: k ? 'Explore' : 'general-purpose', description: k ? 'find OAuth docs' : 'run the test matrix',
          running: true, lastTool: 'Grep', currentTool: k ? 'WebSearch' : 'Bash', started: start - 120_000 * (k + 1),
        })),
      }
    })
    const ORDER: Record<Status, number> = { waiting: 0, idle: 1, busy: 2, ended: 3 }
    sessions.sort((a, b) => ORDER[a.status] - ORDER[b.status])
    const toolMix: Partial<Record<Category, number>> = {}
    for (const t of tools.values()) for (const [k, v] of Object.entries(t)) toolMix[k as Category] = (toolMix[k as Category] ?? 0) + v
    const totalCalls = [...calls.values()].reduce((a, b) => a + b, 0)
    const rows = (mult: number) => [
      { project: 'api-server', model: 'claude-opus-5-5', tokens: 9_400_000 * mult, cost: 41.2 * mult },
      { project: 'site', model: 'claude-fable-5-1', tokens: 2_100_000 * mult, cost: 28.9 * mult },
      { project: 'kaans-terminal', model: 'claude-opus-5-5', tokens: 4_800_000 * mult, cost: 19.4 * mult },
      { project: 'parser', model: 'claude-haiku-5-5', tokens: 3_300_000 * mult, cost: 0.6 * mult },
    ]
    const ranges: LedgerRange[] = (['today', '7d', '30d', '12w', '12m'] as const).map((key, i) => {
      const m = [cost / 90, 5, 19, 40, 130][i]
      const r = rows(m)
      return { key, cost: r.reduce((a, b) => a + b.cost, 0), tokens: r.reduce((a, b) => a + b.tokens, 0), rows: r }
    })
    onSnapshot({
      generatedAt: now,
      day: new Date(now).toISOString().slice(0, 10),
      sessions,
      kpis: {
        sessionsLive: sessions.filter((s) => s.status !== 'ended').length,
        agentsRunning: 3, toolsPerMin: 14.2 + rnd() * 3, tokensPerMin: 380_000 + rnd() * 50_000,
        webCalls: toolMix.web ?? 0, toolCalls: totalCalls, toolErrors: 2, toolOkPct: 97.8, cacheHitPct: 93.1,
        tokensToday: { input: 10_000, output: 400_000, cacheRead: 22_000_000, cacheWrite: 900_000, cacheWrite1h: 0 },
        costToday: cost, burnPerHour: 6.3, cacheSavingsToday: 81.5, toolMix,
      },
      web: [...web],
      files: [...heat.values()].sort((a, b) => b.reads + b.writes - (a.reads + a.writes)),
      clashes: [{ path: clashPath, sessions: ['d1', 'd4'], last: now - 240_000 }],
      terminals: [
        { id: 'demo-1', cwd: '/Users/demo/work/api-server', project: 'api-server', title: 'claude', busy: true, chat: 'd1', started: start, attached: true },
        { id: 'demo-2', cwd: '/Users/demo/work/site', project: 'site', title: 'npm', busy: true, chat: null, started: start, attached: false },
        { id: 'demo-3', cwd: '/Users/demo/work/parser', project: 'parser', title: 'zsh', busy: false, chat: null, started: start, attached: false },
        { id: 'demo-4', cwd: '/Users/demo/work/parser', project: 'parser', title: 'make', busy: true, chat: null, started: start, attached: false },
      ],
      limits: [
        { tool: 'claude', fiveHour: { usedPct: 42, windowMinutes: 300, resetsAt: Math.floor(now / 1000) + 5400 }, weekly: { usedPct: 18, windowMinutes: 10080, resetsAt: Math.floor(now / 1000) + 300000 }, updated: now },
        { tool: 'codex', fiveHour: { usedPct: 66, windowMinutes: 300, resetsAt: Math.floor(now / 1000) + 3600 }, weekly: { usedPct: 47, windowMinutes: 10080, resetsAt: Math.floor(now / 1000) + 200000 }, updated: now },
      ],
      leftovers: [{ pid: 9911, name: 'node', cwd: '/Users/demo/work/site', session: 'demo-old', since: start }],
      runaways: [],
      apps: [
        { pid: 4101, label: 'npm run dev', project: 'site', cwd: '/Users/demo/work/site', cpuPct: 12 + Math.round(Math.abs(Math.sin(now / 9000)) * 20), started: start, terminal: 'demo-2' },
        { pid: 4202, label: 'cargo run', project: 'parser', cwd: '/Users/demo/work/parser', cpuPct: 64, started: start, terminal: null },
        { pid: 4303, label: 'uvicorn', project: 'api-server', cwd: '/Users/demo/work/api-server', cpuPct: 3, started: start, terminal: null },
      ],
      ledger: { ready: true, ranges, daily: Array.from({ length: 30 }, (_, i) => (i === 29 ? cost : 10 + 25 * Math.abs(Math.sin(i * 1.7)))) },
    })
    if (batch.length) onEvents(batch)
  }
  tick()
  setInterval(tick, 1000)
  // dakikada bir etkinlik kovalarını kaydır
  setInterval(() => {
    for (const a of activity.values()) {
      a.shift()
      a.push(0)
    }
  }, 60_000)
}
