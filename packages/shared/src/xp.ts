// XP, level ve rütbe kuralları (OZELLIKLER.md §6.3). Uygulama ve sunucu aynı kodu kullanır: kural tek yerde.
// Girdi yalnızca günlük aktif süredir (token ölçülmez). Sunucu gelince hesap sunucuda yapılır; uygulama aynı
// fonksiyonlarla çevrimdışı önizleme gösterir.

/** Bir günün aktif çalışma süresi */
export interface DayActivity {
  /** Yerel gün, YYYY-MM-DD */
  day: string
  /** Aktif saniye */
  activeSecs: number
}

export const MAX_LEVEL = 60
/** Günün ilk 2 saati ×2 ("Rested") */
export const RESTED_MIN = 120
/** 8 saatten sonra ×0,5 */
export const TIRED_MIN = 480
/** Günün ilk aktif dakikasında +50 */
export const DAILY_BONUS = 50
/** Seri için bir günün sayılması: ≥30 dk */
export const STREAK_DAY_MIN = 30
/** Seri için haftada en az 3 böyle gün */
export const STREAK_DAYS_PER_WEEK = 3
/** On fire: günde 5 saat */
export const ON_FIRE_MIN = 300

/** Bir günün temel XP'si (seri çarpanı hariç): dakika başına 1, ilk 2 saat ×2, 8 saatten sonra ×0,5, +50. */
export function dayBaseXp(activeSecs: number): number {
  const m = Math.floor(Math.max(0, activeSecs) / 60)
  if (m === 0) return 0
  const rested = Math.min(m, RESTED_MIN)
  const normal = Math.max(0, Math.min(m, TIRED_MIN) - RESTED_MIN)
  const tired = Math.max(0, m - TIRED_MIN)
  return DAILY_BONUS + rested * 2 + normal + tired * 0.5
}

/** "Rested" (×2) için bugün kalan dakika */
export function restedLeftMin(activeSecs: number): number {
  return Math.max(0, RESTED_MIN - Math.floor(activeSecs / 60))
}

/** Pazartesi başlayan haftanın anahtarı (o haftanın pazartesi günü, YYYY-MM-DD) */
export function weekOf(day: string): string {
  const [y, mo, d] = day.split('-').map(Number)
  const t = Date.UTC(y, mo - 1, d)
  const dow = (new Date(t).getUTCDay() + 6) % 7 // pazartesi = 0
  return new Date(t - dow * 86_400_000).toISOString().slice(0, 10)
}

function prevWeek(week: string): string {
  const [y, mo, d] = week.split('-').map(Number)
  return new Date(Date.UTC(y, mo - 1, d) - 7 * 86_400_000).toISOString().slice(0, 10)
}

/** Haftanın seri şartını sağlayıp sağlamadığı: en az 3 gün, her biri ≥30 dk */
function qualifies(days: DayActivity[]): boolean {
  return days.filter((d) => d.activeSecs >= STREAK_DAY_MIN * 60).length >= STREAK_DAYS_PER_WEEK
}

/**
 * Seri: bir haftanın çarpanı, kendisinden hemen önceki ardışık şartı sağlayan hafta sayısına göre:
 * her hafta +%10, en fazla +%50.
 */
export function streakBonus(streakWeeks: number): number {
  return Math.min(0.5, 0.1 * Math.max(0, streakWeeks))
}

export interface XpSummary {
  totalXp: number
  level: number
  /** Bu level'da kazanılan XP */
  levelXp: number
  /** Sonraki level için gereken (son level'da 0) */
  nextXp: number
  rank: Rank
  /** Bu haftaya kadar ardışık şartı sağlayan hafta sayısı (bu hafta hariç) */
  streakWeeks: number
  /** Bu haftanın çarpanı (1.0–1.5) */
  multiplier: number
}

/** Bütün günlerden toplam XP (seri çarpanlarıyla) */
export function totalXp(days: DayActivity[], today: string): { total: number; streakWeeks: number } {
  const byWeek = new Map<string, DayActivity[]>()
  for (const d of days) {
    const w = weekOf(d.day)
    if (!byWeek.has(w)) byWeek.set(w, [])
    byWeek.get(w)!.push(d)
  }
  const streakBefore = (week: string) => {
    let n = 0
    for (let w = prevWeek(week); byWeek.has(w) && qualifies(byWeek.get(w)!); w = prevWeek(w)) n++
    return n
  }
  let total = 0
  for (const [w, list] of byWeek) {
    const mult = 1 + streakBonus(streakBefore(w))
    for (const d of list) total += dayBaseXp(d.activeSecs) * mult
  }
  return { total: Math.floor(total), streakWeeks: streakBefore(weekOf(today)) }
}

/** L'den L+1'e geçmek için gereken XP: 2,2 × (L² + 9L), 10'a yuvarlanır */
export function xpToNext(level: number): number {
  if (level >= MAX_LEVEL) return 0
  return Math.round((2.2 * (level * level + 9 * level)) / 10) * 10
}

/** Toplam XP → level ve level içindeki ilerleme */
export function levelOf(total: number): { level: number; levelXp: number; nextXp: number } {
  let level = 1
  let rest = Math.max(0, total)
  while (level < MAX_LEVEL && rest >= xpToNext(level)) {
    rest -= xpToNext(level)
    level++
  }
  return { level, levelXp: level >= MAX_LEVEL ? 0 : rest, nextXp: xpToNext(level) }
}

// ---------------------------------------------------------------- rütbeler: Büyücü Loncası (TASARIM.md §5.8)

export type RankId = 'apprentice' | 'adept' | 'conjurer' | 'runesmith' | 'archmage' | 'oracle' | 'eternal'

export interface Rank {
  id: RankId
  name: string
  /** Bu rütbenin ilk level'ı */
  from: number
  /** Temadan bağımsız rütbe rengi */
  color: string
}

export const RANKS: readonly Rank[] = [
  { id: 'apprentice', name: 'Apprentice', from: 1, color: '#9aa4b2' },
  { id: 'adept', name: 'Adept', from: 5, color: '#5fd38d' },
  { id: 'conjurer', name: 'Conjurer', from: 15, color: '#38c6e8' },
  { id: 'runesmith', name: 'Runesmith', from: 25, color: '#ff9a3c' },
  { id: 'archmage', name: 'Archmage', from: 35, color: '#a77bff' },
  { id: 'oracle', name: 'Oracle', from: 45, color: '#ff5fa8' },
  { id: 'eternal', name: 'Eternal', from: 55, color: '#ffd75e' },
]

export function rankOf(level: number): Rank {
  let r = RANKS[0]
  for (const x of RANKS) if (level >= x.from) r = x
  return r
}

export function summarize(days: DayActivity[], today: string): XpSummary {
  const { total, streakWeeks } = totalXp(days, today)
  const lv = levelOf(total)
  return { totalXp: total, ...lv, rank: rankOf(lv.level), streakWeeks, multiplier: 1 + streakBonus(streakWeeks) }
}

// ---------------------------------------------------------------- heartbeat kredisi (sunucu kuralı)

/** Her atış en fazla 90 sn kredi verir; önceki atıştan boşluk 3 dakikayı aşarsa 60 sn. */
export function heartbeatCredit(gapSecs: number): number {
  if (gapSecs > 180) return 60
  return Math.max(0, Math.min(90, gapSecs))
}
