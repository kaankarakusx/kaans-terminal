// Arkadaşlar, istekler, mesajlar ve canlı olaylar (OZELLIKLER.md §6.2). İstekler Rust'tan gider
// (`social_get` / `social_post`, izin listesi); canlı olaylar `social://live` ile gelir.
import { invoke } from '@tauri-apps/api/core'
import { inTauri } from './engine.svelte'
import { social } from './social.svelte'

export interface Friend {
  name: string
  tag: string
  level: number
  theme: string | null
  online: boolean
  away: boolean
  todaySecs: number | null
  last7: number[] | null
  onFire: boolean
  unread: number
}

export interface Person {
  name: string
  tag: string
}

export interface ChatMessage {
  id: string
  mine: boolean
  /** Çözülmüş metin; çözülemediyse null (`locked`) */
  body: string | null
  at: number
  read?: boolean
  /** Uçtan uca şifreli mesaj bu cihazın anahtarıyla çözülemedi (anahtar değişmiş) */
  locked?: boolean
}

/** Çözülemeyen mesajın yerine gösterilen metin */
export const LOCKED_TEXT = "🔒 can't decrypt: this message was encrypted for an earlier key (app reinstalled?)"

export type Presence = 'online' | 'away' | 'invisible'

/** Köşedeki bildirim: yeni mesaj ya da "is now online" */
export interface Ping {
  key: string
  kind: 'message' | 'online' | 'request'
  who: Person
  body?: string
  at: number
}

export const nameTag = (p: Person) => `${p.name}#${p.tag}`

// Sohbet pencereleri ana pencereye "açığım" der (aynı köken localStorage); açıkken o arkadaşın mesajları için
// köşede bildirim gösterilmez. Pencere 5 sn'de bir tazeler; 15 sn'den eskisi kapanmış sayılır (çökme durumu).
const WIN = 'chat.win.'
export function markChatWindow(k: string): () => void {
  const touch = () => {
    try {
      localStorage.setItem(WIN + k, String(Date.now()))
    } catch {
      /* yok say */
    }
  }
  touch()
  const t = setInterval(touch, 5000)
  const clear = () => {
    clearInterval(t)
    try {
      localStorage.removeItem(WIN + k)
    } catch {
      /* yok say */
    }
  }
  window.addEventListener('beforeunload', clear)
  return clear
}
export function chatWindowOpen(k: string): boolean {
  try {
    return Date.now() - Number(localStorage.getItem(WIN + k) ?? 0) < 15_000
  } catch {
    return false
  }
}

// ---------------------------------------------------------------- tarayıcı demosu

function demoBackend() {
  const t = Date.now()
  let friends: Friend[] = [
    { name: 'ada', tag: '1737', level: 27, theme: 'dracula', online: true, away: false, todaySecs: 3900, last7: [1860, 5040, 8220, 11400, 180, 3360, 3900], onFire: false, unread: 2 },
    { name: 'nightowl', tag: '1000', level: 34, theme: 'night-owl', online: true, away: true, todaySecs: 19800, last7: [7200, 9000, 4000, 12000, 8000, 3000, 19800], onFire: true, unread: 0 },
    { name: 'rustacean', tag: '2474', level: 22, theme: 'gruvbox-dark', online: false, away: false, todaySecs: 5400, last7: [0, 3600, 7200, 0, 5400, 0, 5400], onFire: false, unread: 0 },
  ]
  let incoming: Person[] = [{ name: 'tokyo', tag: '3211' }]
  let outgoing: Person[] = []
  const chats = new Map<string, ChatMessage[]>([
    ['ada#1737', [
      { id: '1', mine: true, body: 'build geçti mi?', at: t - 3.6e6 },
      { id: '2', mine: false, body: 'geçti 🎉 orbit kamerasını da düzelttim', at: t - 3.5e6 },
      { id: '3', mine: false, body: 'bakar mısın?', at: t - 3.4e6 },
    ]],
  ])
  return {
    list: async () => ({ friends, incoming, outgoing }),
    post: async (what: string, body: Record<string, string>) => {
      await new Promise((r) => setTimeout(r, 200))
      const nt = body.nameTag ?? body.to
      const [name, tag] = (nt ?? '').split('#')
      if (what === 'friends.add') {
        if (!/^[\w.-]{3,20}#\d{4}$/.test(nt)) throw new Error('use name#tag, e.g. kaan#4242')
        if (name === 'ghost') throw new Error(`no one called ${nt}`)
        outgoing = [...outgoing, { name, tag }]
        return { status: 'sent' }
      }
      if (what === 'friends.accept') {
        incoming = incoming.filter((p) => nameTag(p) !== nt)
        friends = [...friends, { name, tag, level: 18, theme: 'tokyo-night', online: true, away: false, todaySecs: 2400, last7: [0, 0, 600, 1200, 0, 900, 2400], onFire: false, unread: 0 }]
        return { status: 'friends' }
      }
      if (what === 'friends.remove') {
        incoming = incoming.filter((p) => nameTag(p) !== nt)
        outgoing = outgoing.filter((p) => nameTag(p) !== nt)
        friends = friends.filter((p) => nameTag(p) !== nt)
        return { status: 'none' }
      }
      if (what === 'messages.send') {
        const list = chats.get(nt) ?? []
        const m = { id: String(Date.now()), mine: true, body: body.body, at: Date.now() }
        chats.set(nt, [...list, m])
        return { id: m.id, at: m.at }
      }
      return {}
    },
    messages: async (w: string) => {
      friends = friends.map((f) => (nameTag(f) === w ? { ...f, unread: 0 } : f))
      return { messages: chats.get(w) ?? [], more: false }
    },
  }
}

const demo = inTauri ? null : demoBackend()

async function post(what: string, body: Record<string, unknown>) {
  if (demo) return demo.post(what, body as Record<string, string>)
  return invoke('social_post', { what, body })
}

function errText(e: unknown): string {
  const m = e instanceof Error ? e.message : String(e)
  return m === 'offline' ? "can't reach the server right now" : m
}

class Friends {
  friends = $state.raw<Friend[]>([])
  incoming = $state.raw<Person[]>([])
  outgoing = $state.raw<Person[]>([])
  presence = $state<Presence>('online')
  /** Canlı bağlantı açık mı */
  connected = $state(false)
  /** Açık sohbetler (name#tag) ve mesajları */
  open = $state<string[]>([])
  chats = $state<Record<string, ChatMessage[]>>({})
  pings = $state<Ping[]>([])
  private started = false

  unread = $derived(this.friends.reduce((a, f) => a + f.unread, 0) + this.incoming.length)

  async start() {
    if (this.started) return
    this.started = true
    if (inTauri) {
      const { listen } = await import('@tauri-apps/api/event')
      await listen<Record<string, unknown>>('social://live', (e) => this.onLive(e.payload))
      await listen<{ connected: boolean }>('social://live-state', (e) => {
        this.connected = e.payload.connected
        if (e.payload.connected) void this.load()
      })
    } else {
      this.connected = true
      // Demo: bir süre sonra arkadaştan mesaj ve "is now online"
      setTimeout(() => this.onLive({ type: 'presence', name: 'rustacean', tag: '2474', online: true, away: false, notify: true }), 6000)
      setTimeout(() => this.onLive({ type: 'message', id: 'd1', from: { name: 'ada', tag: '1737' }, body: 'PR açtım, review atar mısın?', at: Date.now() }), 9000)
    }
    await this.load()
  }

  async load() {
    if (!social.state.registered) return
    try {
      const r = demo ? await demo.list() : await invoke<{ friends: Friend[]; incoming: Person[]; outgoing: Person[] }>('social_get', { what: 'friends' })
      this.friends = r.friends
      this.incoming = r.incoming
      this.outgoing = r.outgoing
    } catch {
      /* çevrimdışı: son liste kalır */
    }
  }

  private ping(p: Omit<Ping, 'key' | 'at'>) {
    const key = `${p.kind}:${nameTag(p.who)}:${Date.now()}`
    this.pings = [...this.pings.filter((x) => !(x.kind === p.kind && nameTag(x.who) === nameTag(p.who))), { ...p, key, at: Date.now() }].slice(-4)
    if (p.kind !== 'message') setTimeout(() => this.dismiss(key), 6000)
  }

  dismiss(key: string) {
    this.pings = this.pings.filter((p) => p.key !== key)
  }

  private onLive(ev: Record<string, unknown>) {
    switch (ev.type) {
      case 'message': {
        const from = ev.from as Person
        const k = nameTag(from)
        const m: ChatMessage = { id: String(ev.id), mine: false, body: ev.body == null ? null : String(ev.body), at: Number(ev.at), locked: !!ev.locked }
        if (this.chats[k]) this.chats[k] = [...this.chats[k], m]
        if (this.open.includes(k) || chatWindowOpen(k)) return
        this.friends = this.friends.map((f) => (nameTag(f) === k ? { ...f, unread: f.unread + 1 } : f))
        this.ping({ kind: 'message', who: from, body: m.body ?? LOCKED_TEXT })
        return
      }
      case 'presence': {
        const k = `${ev.name}#${ev.tag}`
        this.friends = this.friends.map((f) => (nameTag(f) === k ? { ...f, online: !!ev.online, away: !!ev.away } : f))
        if (ev.notify) this.ping({ kind: 'online', who: { name: String(ev.name), tag: String(ev.tag) } })
        return
      }
      case 'request':
        this.ping({ kind: 'request', who: ev.from as Person })
        void this.load()
        return
      case 'friend':
        void this.load()
        return
    }
  }

  /** name#tag ile istek gönder; hata metni döner (başarıda null) */
  async add(nt: string): Promise<string | null> {
    try {
      await post('friends.add', { nameTag: nt.trim() })
      await this.load()
      return null
    } catch (e) {
      return errText(e)
    }
  }

  async accept(p: Person) {
    await post('friends.accept', { nameTag: nameTag(p) }).catch(() => {})
    await this.load()
  }

  async remove(p: Person) {
    await post('friends.remove', { nameTag: nameTag(p) }).catch(() => {})
    this.open = this.open.filter((k) => k !== nameTag(p))
    await this.load()
  }

  async setPresence(p: Presence) {
    this.presence = p
    await post('presence', { presence: p }).catch(() => {})
  }

  /** Sohbeti aç: uygulamada kendi penceresinde, tarayıcı demosunda köşedeki kutuda */
  async openChat(p: Person) {
    const k = nameTag(p)
    if (inTauri) {
      this.pings = this.pings.filter((x) => !(x.kind === 'message' && nameTag(x.who) === k))
      this.friends = this.friends.map((f) => (nameTag(f) === k ? { ...f, unread: 0 } : f))
      await invoke('chat_window', { nameTag: k }).catch((e) => console.warn('[chat]', e))
      return
    }
    if (!this.open.includes(k)) this.open = [...this.open, k].slice(-3)
    this.pings = this.pings.filter((x) => !(x.kind === 'message' && nameTag(x.who) === k))
    try {
      const r = demo ? await demo.messages(k) : await invoke<{ messages: ChatMessage[] }>('social_get', { what: 'messages', with: k })
      this.chats[k] = r.messages
      this.friends = this.friends.map((f) => (nameTag(f) === k ? { ...f, unread: 0 } : f))
    } catch {
      this.chats[k] ??= []
    }
  }

  closeChat(k: string) {
    this.open = this.open.filter((x) => x !== k)
  }

  /** Mesaj gönder; hata metni döner */
  async send(k: string, body: string): Promise<string | null> {
    const text = body.trim()
    if (!text) return null
    try {
      const r = (await post('messages.send', { to: k, body: text })) as { id: string; at: number }
      this.chats[k] = [...(this.chats[k] ?? []), { id: r.id, mine: true, body: text, at: r.at }]
      return null
    } catch (e) {
      return errText(e)
    }
  }
}

export const friends = new Friends()
