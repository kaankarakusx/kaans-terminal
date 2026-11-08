// Duyurular ve geri bildirim (OZELLIKLER.md §6.5). Okunmamış: son görülen duyuru kimliği (localStorage).
// Yeni duyuru canlı bağlantıdan gelir ve uygulama arkadayken bile köşede görünür.
import { invoke } from '@tauri-apps/api/core'
import { inTauri } from './engine.svelte'
import { social } from './social.svelte'

export interface Announcement {
  id: string
  title: string
  body: string
  at: number
}

const SEEN = 'news.seen'

const DEMO: Announcement[] = [
  { id: '2', title: 'Signature theme is live', body: "Try Kaan's Signature from the palette icon. Thanks @kaankarakusx for the lime 🍋 — details: https://kaansterminal.karakuslabs.com", at: Date.now() - 3.6e6 },
  { id: '1', title: 'Welcome to the guild', body: 'Pick a hacker name, add friends by name#tag and climb from Apprentice to Eternal.', at: Date.now() - 86_400_000 },
]

class News {
  items = $state.raw<Announcement[]>([])
  seen = $state(0)
  /** Köşede gösterilecek yeni duyurular */
  fresh = $state<Announcement[]>([])
  private started = false

  unread = $derived(this.items.filter((a) => Number(a.id) > this.seen).length)

  async start() {
    if (this.started) return
    this.started = true
    try {
      this.seen = Number(localStorage.getItem(SEEN) ?? 0)
    } catch {
      /* yok say */
    }
    if (inTauri) {
      const { listen } = await import('@tauri-apps/api/event')
      await listen<Record<string, unknown>>('social://live', (e) => {
        const ev = e.payload
        if (ev.type !== 'announcement') return
        const a: Announcement = { id: String(ev.id), title: String(ev.title), body: String(ev.body), at: Number(ev.at) }
        this.items = [a, ...this.items.filter((x) => x.id !== a.id)]
        this.fresh = [...this.fresh, a].slice(-2)
      })
    }
    await this.load()
  }

  async load() {
    if (!inTauri) {
      this.items = DEMO
      return
    }
    if (!social.state.registered) return
    try {
      this.items = (await invoke<{ items: Announcement[] }>('social_get', { what: 'notifications' })).items
    } catch {
      /* çevrimdışı */
    }
  }

  markSeen() {
    const top = Math.max(0, ...this.items.map((a) => Number(a.id)))
    this.seen = top
    this.fresh = []
    try {
      localStorage.setItem(SEEN, String(top))
    } catch {
      /* yok say */
    }
  }

  dismiss(id: string) {
    this.fresh = this.fresh.filter((a) => a.id !== id)
  }

  /** Geri bildirim gönder; hata metni döner */
  async feedback(body: string): Promise<string | null> {
    if (!body.trim()) return 'write something first'
    if (!inTauri) return null
    try {
      await invoke('social_post', { what: 'feedback', body: { body: body.trim() } })
      return null
    } catch (e) {
      const m = String(e)
      return m === 'offline' ? "can't reach the server right now" : m
    }
  }
}

export const news = new News()
