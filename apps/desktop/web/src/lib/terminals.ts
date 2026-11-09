// Ana panelden terminal açma: sohbet listesi, harita, klasör seçici ve kısayol çarkı kullanır.
import * as api from '../term/api'
import { windowFor } from '../term/model.svelte'

/**
 * Oturumu açar: zaten bir pencerede gösteriliyorsa o pencere öne gelir; daha önce bir pencerede gösterildiyse o
 * pencere kaydedilmiş düzeniyle (bölmeler, sekmeler) açılır; değilse yeni pencere.
 */
export async function openTerminal(session: string, title?: string) {
  await once(`t:${session}`, () => api.openWindow({ label: windowFor(session) ?? undefined, session, title }))
}

// Art arda basış: aynı hedef için uçuştaki istek bitmeden ikincisi yok sayılır (iki pencere/iki osascript olmasın).
const inFlight = new Set<string>()
async function once(key: string, f: () => Promise<unknown>) {
  if (inFlight.has(key)) return
  inFlight.add(key)
  try {
    await f()
  } finally {
    inFlight.delete(key)
  }
}

/** `+ new terminal`: klasörde yeni kabuk, kendi penceresinde. */
export async function newTerminal(cwd: string | null) {
  const id = await api.createTerminal(cwd)
  await api.openWindow({ session: id })
  rememberFolder(cwd)
}

const RECENT_KEY = 'ui.recentFolders'

export function recentFolders(): string[] {
  try {
    return JSON.parse(localStorage.getItem(RECENT_KEY) ?? '[]') as string[]
  } catch {
    return []
  }
}

/** Son klasörler listesinden çıkarır (yalnızca listeden; klasöre dokunulmaz). */
export function forgetFolder(cwd: string) {
  try {
    localStorage.setItem(RECENT_KEY, JSON.stringify(recentFolders().filter((c) => c !== cwd)))
  } catch {
    /* yok say */
  }
}

function rememberFolder(cwd: string | null) {
  if (!cwd) return
  try {
    const list = [cwd, ...recentFolders().filter((c) => c !== cwd)].slice(0, 30)
    localStorage.setItem(RECENT_KEY, JSON.stringify(list))
  } catch {
    /* yalnızca bu oturum */
  }
}

/**
 * Sohbeti aç (kart, harita, bildirim): bizim terminalimizdeyse o pencere; iTerm2/Terminal'deyse o sekme öne gelir;
 * ikisi de değilse kart sohbet listesinde gösterilir.
 */
export async function openChat(s: { id: string; terminal: string | null; pid: number | null; title: string }) {
  const { focus } = await import('./focus.svelte')
  if (s.terminal) return openTerminal(s.terminal, s.title)
  await once(`c:${s.id}`, async () => {
    // Engine eşleşmesi henüz gelmediyse (ilk saniyeler): ön plandaki süreci bu sohbet olan kendi terminalimiz
    if (s.pid) {
      const own = (await api.termState().catch(() => null))?.sessions.find((t) => t.fgPid === s.pid)
      if (own) return openTerminal(own.id, s.title)
    }
    if (s.pid && (await api.focusExternal(s.pid).catch(() => false))) return
    focus.session(s.id)
  })
}
