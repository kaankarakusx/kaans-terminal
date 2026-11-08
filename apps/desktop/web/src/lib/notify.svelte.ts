// Uyarılar (OZELLIKLER.md §2.5):
// - "needs you": bir sohbet onay ya da soru bekliyor → köşede bildirim, cevaplanana kadar kalır; isteğe bağlı ses;
//   uygulama öndeyken değilse işletim sistemi bildirimi.
// - "same file": iki sohbet son 20 dakikada aynı dosyaya yazdı → durum bitene kadar köşede kalır.
import { untrack } from 'svelte'
import { engine, inTauri } from './engine.svelte'
import { power } from './power.svelte'
import { attention, unlock, warning } from './sfx'
import type { Snapshot } from './types'

const SOUND_KEY = 'ui.sound'

export interface Toast {
  key: string
  kind: 'waiting' | 'clash'
  session?: string
  title: string
  detail: string
  since: number
}

function readSound() {
  try {
    return localStorage.getItem(SOUND_KEY) === 'on'
  } catch {
    return false
  }
}

class Notifier {
  /** Ses varsayılan kapalı ("isteğe bağlı ses"). */
  sound = $state(readSound())
  /** Kullanıcının kapattığı bildirimler; durum değişince yeniden gösterilebilir. */
  dismissed = $state<Record<string, true>>({})
  toasts = $state.raw<Toast[]>([])

  private prevWaiting = new Set<string>()
  private prevClashes = new Set<string>()
  private primed = false

  toggleSound() {
    this.sound = !this.sound
    if (this.sound) unlock()
    try {
      localStorage.setItem(SOUND_KEY, this.sound ? 'on' : 'off')
    } catch {
      /* yalnızca bu oturum */
    }
  }

  dismiss(key: string) {
    this.dismissed[key] = true
    this.toasts = this.toasts.filter((t) => t.key !== key)
  }

  /** Her snapshot'ta çağrılır: bildirimleri günceller, yeni durumlarda ses/sistem bildirimi verir. */
  update(snap: Snapshot) {
    const waiting = snap.sessions.filter((s) => s.status === 'waiting')
    const waitingIds = new Set(waiting.map((s) => s.id))
    const clashKeys = new Set(snap.clashes.map((c) => `clash:${c.path}`))

    // Biten durumların "kapatıldı" işareti silinir; tekrar olursa yeniden gösterilir.
    for (const k of Object.keys(this.dismissed)) {
      const live = k.startsWith('clash:') ? clashKeys.has(k) : waitingIds.has(k.slice('wait:'.length))
      if (!live) delete this.dismissed[k]
    }

    const fresh = waiting.filter((s) => !this.prevWaiting.has(s.id))
    const freshClash = snap.clashes.filter((c) => !this.prevClashes.has(`clash:${c.path}`))
    // İlk snapshot'ta (uygulama açılırken) zaten bekleyenler için ses çalınmaz.
    if (this.primed) {
      if (fresh.length) {
        if (this.sound) attention()
        if (power.mode !== 'foreground') void this.system(fresh.map((s) => s.title))
      } else if (freshClash.length && this.sound) {
        warning()
      }
    }
    this.primed = true
    this.prevWaiting = waitingIds
    this.prevClashes = clashKeys

    const byId = new Map(snap.sessions.map((s) => [s.id, s]))
    const toasts: Toast[] = [
      ...waiting.map((s) => ({
        key: `wait:${s.id}`,
        kind: 'waiting' as const,
        session: s.id,
        title: s.title,
        detail: s.waitingFor ?? 'needs approval',
        since: s.lastActivity,
      })),
      ...snap.clashes.map((c) => ({
        key: `clash:${c.path}`,
        kind: 'clash' as const,
        session: c.sessions[0],
        title: c.path.split('/').at(-1) ?? c.path,
        detail: c.sessions.map((id) => byId.get(id)?.title ?? id.slice(0, 8)).join(' & '),
        since: c.last,
      })),
    ]
    this.toasts = toasts.filter((t) => !this.dismissed[t.key])
  }

  private async system(titles: string[]) {
    if (!inTauri) return
    try {
      const n = await import('@tauri-apps/plugin-notification')
      let ok = await n.isPermissionGranted()
      if (!ok) ok = (await n.requestPermission()) === 'granted'
      if (!ok) return
      n.sendNotification({
        title: titles.length === 1 ? 'Needs you' : `${titles.length} chats need you`,
        body: titles.join(' · '),
      })
    } catch (e) {
      console.warn('[notify]', e)
    }
  }
}

export const notifier = new Notifier()

// Snapshot değiştikçe bildirimleri güncelle (bileşen yaşam döngüsünden bağımsız).
$effect.root(() => {
  $effect(() => {
    const snap = engine.snapshot
    // Yalnızca snapshot'a bağlı; update içinde okunan/yazılan durum efekti yeniden tetiklemesin.
    if (snap) untrack(() => notifier.update(snap))
  })
})
