// Ana panelden terminal açma: sohbet listesi, harita, klasör seçici ve kısayol çarkı kullanır.
import * as api from '../term/api'
import { windowFor } from '../term/model.svelte'

/**
 * Oturumu açar: zaten bir pencerede gösteriliyorsa o pencere öne gelir; daha önce bir pencerede gösterildiyse o
 * pencere kaydedilmiş düzeniyle (bölmeler, sekmeler) açılır; değilse yeni pencere.
 */
export async function openTerminal(session: string, title?: string) {
  await api.openWindow({ label: windowFor(session) ?? undefined, session, title })
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
  if (s.pid && (await api.focusExternal(s.pid).catch(() => false))) return
  focus.session(s.id)
}
