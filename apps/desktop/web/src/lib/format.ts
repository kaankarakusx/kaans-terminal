/** 1234 → 1.2k, 1_500_000 → 1.5M */
export function compact(n: number): string {
  if (n < 1000) return String(Math.round(n))
  if (n < 1_000_000) return `${(n / 1000).toFixed(n < 10_000 ? 1 : 0)}k`
  if (n < 1_000_000_000) return `${(n / 1_000_000).toFixed(n < 10_000_000 ? 1 : 0)}M`
  return `${(n / 1_000_000_000).toFixed(1)}B`
}

/** Geçen süre: 12s, 4m, 2h, 3d */
export function ago(ms: number): string {
  const s = Math.max(0, Math.floor(ms / 1000))
  if (s < 60) return `${s}s`
  const m = Math.floor(s / 60)
  if (m < 60) return `${m}m`
  const h = Math.floor(m / 60)
  if (h < 48) return `${h}h`
  return `${Math.floor(h / 24)}d`
}

export function clock(ts: number): string {
  return new Date(ts).toLocaleTimeString('en-GB', { hour12: false })
}

/** claude-opus-5-5 → opus-5-5 */
export function shortModel(m: string | null): string {
  return m ? m.replace(/^claude-/, '') : '—'
}

export function pct(v: number | null): string {
  return v == null ? '—' : `${Math.round(v)}%`
}

/** 0.42 → $0.42, 111.4 → $111, 1234 → $1.2k */
export function usd(v: number): string {
  if (v >= 1000) return `$${(v / 1000).toFixed(1)}k`
  if (v >= 100) return `$${Math.round(v)}`
  return `$${v.toFixed(2)}`
}

/** claude-opus-5-5 → opus 5.5, gpt-5.5 → gpt-5.5 */
export function prettyModel(m: string | null): string {
  if (!m) return '—'
  const c = m.match(/^claude-([a-z]+)-(\d+)(?:-(\d+))?/)
  if (c) return `${c[1]} ${c[2]}${c[3] && c[3].length <= 2 ? `.${c[3]}` : ''}`
  return m
}

/** 734 → 12m, 7300 → 2h 1m (saniye cinsinden süre) */
export function duration(sec: number): string {
  const s = Math.max(0, Math.floor(sec))
  if (s < 60) return `${s}s`
  const m = Math.floor(s / 60)
  if (m < 60) return `${m}m`
  return `${Math.floor(m / 60)}h ${m % 60}m`
}

/** Süre sayacı: 754000 → 12:34, 4000000 → 1:06:40 */
export function stopwatch(ms: number): string {
  const s = Math.max(0, Math.floor(ms / 1000))
  const h = Math.floor(s / 3600)
  const mm = String(Math.floor((s % 3600) / 60)).padStart(2, '0')
  const ss = String(s % 60).padStart(2, '0')
  return h ? `${h}:${mm}:${ss}` : `${mm}:${ss}`
}

/** /Users/kaan/x → ~/x */
export function tildePath(p: string): string {
  return p.replace(/^\/Users\/[^/]+/, '~').replace(/^\/home\/[^/]+/, '~').replace(/^[A-Za-z]:\\Users\\[^\\]+/, '~')
}
