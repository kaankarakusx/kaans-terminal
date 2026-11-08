// Sosyal sunucu durumu (MIMARI.md §6). İstekler Rust'tan gider (token sayfaya girmez); sayfa yalnızca izin
// listesindeki uçları ister (`social_get`). Sunucuya ulaşılamazsa uygulama yerel önizlemeyle çalışmaya devam eder.
import { invoke } from '@tauri-apps/api/core'
import type { XpSummary } from '@kaans/shared/xp'
import { inTauri } from './engine.svelte'

export interface SocialState {
  server: string
  registered: boolean
  name: string | null
  tag: string | null
}

/** GET me/status */
export interface MeStatus {
  name: string
  tag: string
  theme: string | null
  hideTime: boolean
  hideName: boolean
  summary: XpSummary
  weekXp: number
  todaySecs: number
  weekSecs: number
  /** Son 7 gün, saniye, eskiden yeniye */
  last7: number[]
  restedLeftMin: number
  onFire: boolean
}

export interface BoardRow {
  name: string
  tag: string
  you: boolean
  /** Benimle arkadaşlık durumu (sıralamadan tek tıkla istek) */
  friend?: 'self' | 'none' | 'sent' | 'received' | 'friends'
  level: number
  xp: number
  theme: string | null
  online: boolean
  onFire: boolean
  todaySecs: number | null
  last7: number[] | null
}

export interface Board {
  scope: 'week' | 'all'
  working: number
  rows: BoardRow[]
}

const LATER = 'social.later'
export const NAME_RE = /^[A-Za-z0-9_.-]{3,20}$/

// ---------------------------------------------------------------- tarayıcı demosu: sahte sunucu

function demoServer() {
  let me: SocialState = { server: 'demo', registered: false, name: null, tag: null }
  const people: BoardRow[] = [
    ['nightowl', 34, 3840, 'night-owl', true, true], ['ada', 27, 2610, 'dracula', true, false],
    ['rustacean', 22, 1980, 'gruvbox-dark', false, false], ['tokyo', 18, 1420, 'tokyo-night', true, false],
    ['mocha', 12, 960, 'catppuccin-mocha', false, false], ['Anonymous', 9, 520, null, false, false],
  ].map(([name, level, xp, theme, online, fire], i) => ({
    name: name as string, tag: name === 'Anonymous' ? '' : String(1000 + i * 737).slice(0, 4), you: false,
    level: level as number, xp: xp as number, theme: theme as string | null, online: online as boolean,
    onFire: fire as boolean, todaySecs: name === 'Anonymous' ? null : (fire ? 330 : 40 + i * 25) * 60,
    last7: name === 'Anonymous' ? null : Array.from({ length: 7 }, (_, d) => ((d * 53 + i * 31) % 240) * 60),
  }))
  return {
    state: async () => me,
    register: async (name: string) => {
      await new Promise((r) => setTimeout(r, 600))
      if (name.toLowerCase() === 'taken') throw new Error('this name is too popular, pick another')
      me = { server: 'demo', registered: true, name, tag: '4242' }
      return me
    },
    board: async (scope: 'week' | 'all'): Promise<Board> => {
      const you: BoardRow = {
        name: me.name ?? 'you', tag: me.tag ?? '', you: true, level: 25, xp: scope === 'week' ? 2100 : 18000,
        theme: null, online: true, onFire: false, todaySecs: 140 * 60, last7: [160, 200, 0, 240, 180, 60, 140].map((m) => m * 60),
      }
      const rows = [...people.map((p) => ({ ...p, xp: scope === 'week' ? p.xp : p.xp * 7 })), you].sort((a, b) => b.xp - a.xp)
      return { scope, working: rows.filter((r) => r.online).length, rows }
    },
  }
}

const demo = inTauri ? null : demoServer()

class Social {
  state = $state<SocialState>({ server: '', registered: false, name: null, tag: null })
  status = $state.raw<MeStatus | null>(null)
  boards = $state.raw<{ week?: Board; all?: Board }>({})
  /** Son istek sunucuya ulaşamadı */
  offline = $state(false)
  /** "Sonra" denmiş: hacker name penceresi kendiliğinden açılmaz */
  later = $state(false)
  /** Hacker name penceresi */
  picking = $state(false)
  private started = false

  async start() {
    if (this.started) return
    this.started = true
    try {
      this.later = localStorage.getItem(LATER) === '1'
    } catch {
      /* yok say */
    }
    this.state = demo ? await demo.state() : await invoke<SocialState>('social_state')
    if (inTauri) {
      const { listen } = await import('@tauri-apps/api/event')
      await listen('social://status', () => {
        this.offline = false
        void this.refresh()
      })
      await listen<string>('social://error', (e) => {
        this.offline = e.payload === 'offline'
        if (e.payload === 'unauthorized') void this.lost()
      })
    }
    if (this.state.registered) await this.refresh()
    else if (!this.later) await this.probe()
  }

  /** Sunucu ayakta mı (kayıtsızken hacker name penceresini açmadan önce) */
  private async probe() {
    if (demo) {
      this.picking = true
      return
    }
    try {
      await invoke('social_get', { what: 'leaderboard.week' })
    } catch (e) {
      // Kayıtsızken 401 beklenir: sunucu ayakta demektir
      this.offline = String(e) === 'offline'
    }
    if (!this.offline) this.picking = true
  }

  async refresh() {
    if (!this.state.registered) return
    try {
      if (!demo) this.status = await invoke<MeStatus>('social_get', { what: 'status' })
      this.offline = false
    } catch (e) {
      this.offline = String(e) === 'offline'
      if (String(e) === 'unauthorized') await this.lost()
    }
  }

  /**
   * Sunucu hesabı tanımadı (başka sunucuda açılmış ya da silinmiş): Rust hesabı bıraktı. Durum yenilenir ve hacker
   * name penceresi açılır ("later" seçilmiş olsa bile: kullanıcı aslında kayıtlı sanıyordu).
   */
  private async lost() {
    if (demo) return
    this.state = await invoke<SocialState>('social_state')
    if (!this.state.registered) {
      this.status = null
      this.picking = true
    }
  }

  async board(scope: 'week' | 'all'): Promise<void> {
    if (!this.state.registered) return
    try {
      const b = demo ? await demo.board(scope) : await invoke<Board>('social_get', { what: `leaderboard.${scope}` })
      this.boards = { ...this.boards, [scope]: b }
      this.offline = false
    } catch (e) {
      this.offline = String(e) === 'offline'
      if (String(e) === 'unauthorized') await this.lost()
    }
  }

  /** Hacker name seç (ilk kez) ya da değiştir. Hata mesajı fırlatır. */
  async choose(name: string): Promise<void> {
    if (!NAME_RE.test(name)) throw new Error('3–20 letters, digits, _ . -')
    try {
      if (demo) this.state = await demo.register(name)
      else this.state = await invoke<SocialState>(this.state.registered ? 'social_rename' : 'social_register', { name })
    } catch (e) {
      const m = e instanceof Error ? e.message : String(e)
      throw new Error(m === 'offline' ? "can't reach the server right now" : m)
    }
    // Pencere açık kalır: "welcome name#tag" gösterilir, kullanıcı "enter" ile kapatır
    await this.refresh()
  }

  dismiss() {
    this.picking = false
    this.later = true
    try {
      localStorage.setItem(LATER, '1')
    } catch {
      /* yok say */
    }
  }

  /** Seçili tema sunucuya bir sonraki heartbeat'le gider */
  setTheme(id: string) {
    if (inTauri) void invoke('social_set_theme', { theme: id }).catch(() => {})
  }
}

export const social = new Social()
