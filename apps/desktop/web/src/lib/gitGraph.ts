// Commit grafiği için şerit (lane) yerleşimi. Her satır: commit'in sütunu, üstten gelen ve alta giden şeritler.
// Çizim: üstteki şerit commit'i bekliyorsa commit'e iner, değilse aynı hash'in alttaki şeridine devam eder;
// commit'ten her ebeveynin alttaki şeridine bir çizgi çıkar.
import type { GitCommit } from './types'

export interface GraphRow {
  commit: GitCommit
  col: number
  /** Satırın üstündeki şeritler (her biri beklediği hash) */
  above: (string | null)[]
  /** Satırın altındaki şeritler */
  below: (string | null)[]
}

export function layoutGraph(commits: GitCommit[]): { rows: GraphRow[]; width: number } {
  let lanes: (string | null)[] = []
  const rows: GraphRow[] = []
  let width = 1
  for (const c of commits) {
    const above = lanes.slice()
    let col = lanes.indexOf(c.hash)
    if (col < 0) {
      col = lanes.indexOf(null)
      if (col < 0) col = lanes.length
    }
    // Bu commit'i bekleyen diğer şeritler burada biter (birleşme)
    const next = lanes.map((h, i) => (h === c.hash && i !== col ? null : h))
    next[col] = c.parents[0] ?? null
    for (const p of c.parents.slice(1)) {
      if (next.includes(p)) continue
      const free = next.indexOf(null)
      if (free >= 0 && free !== col) next[free] = p
      else next.push(p)
    }
    // Sondaki boş şeritleri kırp
    while (next.length && next[next.length - 1] === null) next.pop()
    lanes = next
    width = Math.max(width, above.length, next.length, col + 1)
    rows.push({ commit: c, col, above, below: next.slice() })
  }
  return { rows, width }
}
