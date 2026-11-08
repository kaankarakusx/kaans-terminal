// Terminal komutları (Rust: src-tauri/src/term.rs). Tauri dışında (tarayıcıda `npm run dev`) girdiyi yankılayan
// sahte bir kabukla çalışır; böylece terminal arayüzü de tarayıcıda görülebilir.
import { Channel, invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { inTauri } from '../lib/engine.svelte'

export interface SessionInfo {
  id: string
  cwd: string
  command: string[]
  pid: number | null
  fgPid: number | null
  fgName: string | null
  busy: boolean
  started: number
  cols: number
  rows: number
  attached: number
}

export interface Leftover {
  pid: number
  name: string
  cwd: string
  session: string
  since: number
}

export interface TermState {
  sessions: SessionInfo[]
  leftovers: Leftover[]
}

// ───────────────────────── tarayıcı demosu ─────────────────────────
const demo = {
  sessions: new Map<string, SessionInfo>(),
  sinks: new Map<number, { id: string; out: (b: Uint8Array) => void; line: string }>(),
  next: 1,
}
const enc = new TextEncoder()
const PROMPT = '\x1b[38;2;248;128;8m❯\x1b[0m '

function demoState(): TermState {
  return { sessions: [...demo.sessions.values()], leftovers: [] }
}

// ───────────────────────── API ─────────────────────────

export async function termState(): Promise<TermState> {
  if (!inTauri) return demoState()
  return invoke<TermState>('term_state')
}

export async function createTerminal(cwd: string | null, cols = 100, rows = 30, command?: string[]): Promise<string> {
  if (!inTauri) {
    const id = `demo-${demo.next++}`
    demo.sessions.set(id, {
      id, cwd: cwd ?? '/Users/demo', command: command ?? ['zsh'], pid: 4000 + demo.next, fgPid: null, fgName: 'zsh',
      busy: false, started: Date.now(), cols, rows, attached: 0,
    })
    return id
  }
  return invoke<string>('term_create', { cwd, command: command ?? null, cols, rows })
}

/** Bölmeyi oturuma bağlar; çıktı (önce geçmiş) `onOutput`'a gelir. Dönen kimlik girdi/boyut için. */
export async function attach(id: string, onOutput: (bytes: Uint8Array) => void): Promise<number> {
  if (!inTauri) {
    const a = demo.next++
    demo.sinks.set(a, { id, out: onOutput, line: '' })
    const s = demo.sessions.get(id)
    setTimeout(() => {
      onOutput(enc.encode(`\x1b[2mdemo shell in ${s?.cwd ?? '~'} — keys echo locally\x1b[0m\r\n${PROMPT}`))
    }, 50)
    return a
  }
  const ch = new Channel<ArrayBuffer>()
  ch.onmessage = (buf) => onOutput(new Uint8Array(buf))
  return invoke<number>('term_attach', { id, onOutput: ch })
}

export async function input(attachId: number, data: string): Promise<void> {
  if (!inTauri) {
    const s = demo.sinks.get(attachId)
    if (!s) return
    for (const ch of data) {
      if (ch === '\r') {
        const cmd = s.line.trim()
        s.line = ''
        s.out(enc.encode(`\r\n${cmd ? `\x1b[2m(demo) ${cmd}\x1b[0m\r\n` : ''}${PROMPT}`))
      } else if (ch === '\x7f') {
        if (s.line) {
          s.line = s.line.slice(0, -1)
          s.out(enc.encode('\b \b'))
        }
      } else if (ch >= ' ') {
        s.line += ch
        s.out(enc.encode(ch))
      }
    }
    return
  }
  return invoke('term_input', { attach: attachId, data })
}

export async function resize(attachId: number, cols: number, rows: number): Promise<void> {
  if (!inTauri) return
  return invoke('term_resize', { attach: attachId, cols, rows })
}

export async function detach(attachId: number): Promise<void> {
  if (!inTauri) {
    demo.sinks.delete(attachId)
    return
  }
  return invoke('term_detach', { attach: attachId })
}

export async function killTerminal(id: string): Promise<void> {
  if (!inTauri) {
    demo.sessions.delete(id)
    return
  }
  return invoke('term_kill', { id })
}

export async function killLeftover(pid: number): Promise<void> {
  if (!inTauri) return
  return invoke('term_kill_leftover', { pid })
}

export async function openWindow(opts: { label?: string; session?: string; title?: string }): Promise<string> {
  if (!inTauri) {
    window.open(`${location.pathname}#term=demo-win&open=${opts.session ?? ''}`, '_blank')
    return 'demo-win'
  }
  return invoke<string>('term_open_window', {
    label: opts.label ?? null,
    session: opts.session ?? null,
    title: opts.title ?? null,
  })
}

export async function lastReply(terminal: string): Promise<string | null> {
  if (!inTauri) return 'Demo answer: the hexagons now glow and waiting sessions park at the edge.'
  return invoke<string | null>('chat_last_reply', { terminal })
}

export async function openPath(path: string, cwd: string | null): Promise<void> {
  if (!inTauri) return
  return invoke('open_path', { path, cwd })
}

export async function focusExternal(pid: number): Promise<boolean> {
  if (!inTauri) return false
  return invoke<boolean>('focus_external', { pid })
}

export async function dangerKill(pid: number): Promise<void> {
  if (!inTauri) return
  return invoke('danger_kill', { pid })
}

export function onSessions(fn: (s: TermState) => void): Promise<UnlistenFn> {
  if (!inTauri) {
    const t = setInterval(() => fn(demoState()), 1000)
    return Promise.resolve(() => clearInterval(t))
  }
  return listen<TermState>('host://sessions', (e) => fn(e.payload))
}

/** `ended`: End Session ile bitirildi (haritada patlama); false = kabuk kendisi kapandı. */
export function onExit(fn: (id: string, ended: boolean) => void): Promise<UnlistenFn> {
  if (!inTauri) return Promise.resolve(() => {})
  return listen<{ id: string; ended: boolean }>('host://exit', (e) => fn(e.payload.id, e.payload.ended))
}

/** Klasörün git dalı (yoksa null). */
export async function gitBranch(cwd: string): Promise<string | null> {
  if (!inTauri) return 'master'
  return invoke<string | null>('git_branch', { cwd })
}
