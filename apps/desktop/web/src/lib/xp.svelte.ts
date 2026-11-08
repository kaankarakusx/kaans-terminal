// XP durumu: Rust günlük aktif süreyi verir (`activity_days`, `xp://activity`), XP/level/rütbe ortak paketle
// (`@kaans/shared/xp`) burada hesaplanır. Sunucu gelince aynı hesap sunucuda yapılır, bu çevrimdışı önizlemedir.
// Level atlayınca kutlama sırası (`celebrations`) dolar; uygulama öndeyken gösterilir.
import { invoke } from '@tauri-apps/api/core'
import { ON_FIRE_MIN, RANKS, restedLeftMin, summarize, weekOf, type DayActivity, type Rank } from '@kaans/shared/xp'
import { inTauri } from './engine.svelte'
import { now } from './now.svelte'

const SEEN = 'xp.seenLevel'

export interface Celebration {
  kind: 'level' | 'rank'
  level: number
  rank: Rank
}

function localDay(d = new Date()): string {
  const p = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`
}

/** Tarayıcı demosu: ~6 haftalık sahte geçmiş */
function demoDays(): DayActivity[] {
  const out: DayActivity[] = []
  const now = new Date()
  for (let i = 45; i >= 0; i--) {
    const d = new Date(now.getTime() - i * 86_400_000)
    const wd = d.getDay()
    const mins = wd === 0 ? 0 : wd === 6 ? 40 : 140 + ((i * 37) % 200)
    if (mins) out.push({ day: localDay(d), activeSecs: (i === 0 ? 312 : mins) * 60 })
  }
  return out
}

class Xp {
  days = $state.raw<DayActivity[]>([])
  celebrations = $state<Celebration[]>([])
  private started = false

  /** Yerel gün; gece yarısı geçince kendiliğinden değişir */
  today = $derived(localDay(new Date(now.value)))
  summary = $derived(summarize(this.days, this.today))
  todaySecs = $derived(this.days.find((d) => d.day === this.today)?.activeSecs ?? 0)
  weekSecs = $derived.by(() => {
    const w = weekOf(this.today)
    return this.days.filter((d) => weekOf(d.day) === w).reduce((a, d) => a + d.activeSecs, 0)
  })
  restedLeft = $derived(restedLeftMin(this.todaySecs))
  onFire = $derived(this.todaySecs >= ON_FIRE_MIN * 60)
  /** Son 7 gün (bugün dahil), eskiden yeniye: dakika */
  last7 = $derived.by(() => {
    const out: { day: string; min: number }[] = []
    for (let i = 6; i >= 0; i--) {
      const day = localDay(new Date(now.value - i * 86_400_000))
      out.push({ day, min: Math.floor((this.days.find((d) => d.day === day)?.activeSecs ?? 0) / 60) })
    }
    return out
  })

  async start() {
    if (this.started) return
    this.started = true
    if (!inTauri) {
      this.set(demoDays())
      return
    }
    const { listen } = await import('@tauri-apps/api/event')
    await listen<DayActivity[]>('xp://activity', (e) => this.set(e.payload))
    this.set(await invoke<DayActivity[]>('activity_days'))
  }

  private set(days: DayActivity[]) {
    this.days = days
    this.checkLevel()
  }

  /** Daha önce görülen level'dan yüksekse kutlama sırasına ekler. İlk çalıştırmada kutlamaz. */
  private checkLevel() {
    const { level } = this.summary
    let seen: number | null = null
    try {
      const v = localStorage.getItem(SEEN)
      seen = v == null ? null : Number(v)
    } catch {
      /* depolama yok: kutlama da yok */
      return
    }
    if (seen != null && level > seen) {
      const oldRank = rankFor(seen)
      const rank = rankFor(level)
      this.celebrations.push(rank.id !== oldRank.id ? { kind: 'rank', level, rank } : { kind: 'level', level, rank })
    }
    try {
      localStorage.setItem(SEEN, String(level))
    } catch {
      /* yok say */
    }
  }

  /** Geliştirme/deneme: kutlamayı elle tetikler */
  preview(kind: 'level' | 'rank') {
    const { level, rank } = this.summary
    this.celebrations.push({ kind, level, rank })
  }
}

function rankFor(level: number): Rank {
  let r = RANKS[0]
  for (const x of RANKS) if (level >= x.from) r = x
  return r
}

export const xp = new Xp()
