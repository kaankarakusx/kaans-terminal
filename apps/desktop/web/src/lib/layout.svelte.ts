// Panel düzeni (OZELLIKLER.md §2.4): paneller şeritlerinden tutularak taşınır, aradaki çizgilerden
// boyutlandırılır, gizlenir ("panels" menüsünden geri gelir) ve temizlenir. Düzen kaydedilir.
// orbit.map sabittir (ortada); diğer paneller üç alana yerleşir: sol, sağ, alt.

export type PanelId =
  | 'sessions'
  | 'research.feed'
  | 'files.heat'
  | 'tool.mix'
  | 'session.log'
  | 'workers'
  | 'spend.ledger'
  | 'your.call'

export type Area = 'left' | 'right' | 'bottom'

export const ALL_PANELS: PanelId[] = [
  'sessions', 'research.feed', 'files.heat', 'tool.mix', 'session.log', 'workers', 'spend.ledger', 'your.call',
]

/** Temizlenebilir paneller (sohbet listesi temizlenmez). */
export const CLEARABLE = new Set<PanelId>(ALL_PANELS.filter((p) => p !== 'sessions'))

interface Saved {
  version: 1
  areas: Record<Area, PanelId[]>
  hidden: PanelId[]
  weights: Partial<Record<PanelId, number>>
  /** sol ve sağ: pencere genişliğinin %'si; alt: yüksekliğin %'si */
  sizes: Record<Area, number>
  /** panel → temizlendiği an */
  cleared: Partial<Record<PanelId, number>>
  /** toplam sayan panellerin temizlendiği andaki değerleri (files.heat, tool.mix) */
  baselines: Partial<Record<PanelId, Record<string, number>>>
}

const KEY = 'ui.layout'

function defaults(): Saved {
  return {
    version: 1,
    areas: {
      left: ['sessions'],
      right: ['research.feed', 'files.heat', 'tool.mix'],
      bottom: ['session.log', 'workers', 'spend.ledger', 'your.call'],
    },
    hidden: [],
    weights: { 'session.log': 1.6, workers: 1, 'spend.ledger': 0.95, 'your.call': 1 },
    sizes: { left: 22, right: 23, bottom: 27 },
    cleared: {},
    baselines: {},
  }
}

/** Kayıtlı düzen eksik ya da bozuksa (yeni panel eklendiyse) varsayılanla tamamlanır. */
function load(): Saved {
  const d = defaults()
  try {
    const raw = JSON.parse(localStorage.getItem(KEY) ?? 'null') as Saved | null
    if (!raw || raw.version !== 1) return d
    const seen = new Set<PanelId>()
    const areas = { left: [], right: [], bottom: [] } as Record<Area, PanelId[]>
    for (const a of ['left', 'right', 'bottom'] as Area[]) {
      for (const id of raw.areas?.[a] ?? []) {
        if (ALL_PANELS.includes(id) && !seen.has(id)) {
          areas[a].push(id)
          seen.add(id)
        }
      }
    }
    for (const a of ['left', 'right', 'bottom'] as Area[]) {
      for (const id of d.areas[a]) if (!seen.has(id)) areas[a].push(id)
    }
    return { ...d, ...raw, areas, hidden: (raw.hidden ?? []).filter((h) => ALL_PANELS.includes(h)) }
  } catch {
    return d
  }
}

export interface DropTarget {
  area: Area
  index: number
}

class Layout {
  s = $state<Saved>(load())
  /** Taşınan panel ve hedefi (hayalet kopya için). */
  drag = $state<{ id: PanelId; x: number; y: number; target: DropTarget | null } | null>(null)

  visible(area: Area): PanelId[] {
    return this.s.areas[area].filter((id) => !this.s.hidden.includes(id))
  }

  isHidden(id: PanelId) {
    return this.s.hidden.includes(id)
  }

  save() {
    try {
      localStorage.setItem(KEY, JSON.stringify(this.s))
    } catch {
      /* depolama yoksa düzen yalnızca bu oturumda kalır */
    }
  }

  hide(id: PanelId) {
    if (!this.isHidden(id)) this.s.hidden.push(id)
    this.save()
  }

  show(id: PanelId) {
    this.s.hidden = this.s.hidden.filter((h) => h !== id)
    this.save()
  }

  toggle(id: PanelId) {
    if (this.isHidden(id)) this.show(id)
    else this.hide(id)
  }

  move(id: PanelId, to: DropTarget) {
    let index = to.index
    for (const a of ['left', 'right', 'bottom'] as Area[]) {
      const i = this.s.areas[a].indexOf(id)
      if (i >= 0) {
        this.s.areas[a].splice(i, 1)
        if (a === to.area && i < index) index--
      }
    }
    this.s.areas[to.area].splice(Math.max(0, Math.min(index, this.s.areas[to.area].length)), 0, id)
    this.save()
  }

  /** Alan boyutu; `persist` false iken sürükleme sırasında yalnızca bellekte değişir. */
  setSize(area: Area, pct: number, persist = false) {
    const [min, max] = area === 'bottom' ? [12, 60] : [12, 45]
    this.s.sizes[area] = Math.min(max, Math.max(min, pct))
    if (persist) this.save()
  }

  weight(id: PanelId) {
    return this.s.weights[id] ?? 1
  }

  /** İki komşu panel arasındaki çizgi: toplam ağırlık korunur. */
  setWeights(a: PanelId, wa: number, b: PanelId, wb: number, persist = false) {
    const MIN = 0.15
    if (wa < MIN || wb < MIN) return
    this.s.weights[a] = wa
    this.s.weights[b] = wb
    if (persist) this.save()
  }

  clear(id: PanelId, baseline?: Record<string, number>) {
    this.s.cleared[id] = Date.now()
    if (baseline) this.s.baselines[id] = baseline
    this.save()
  }

  unclear(id: PanelId) {
    delete this.s.cleared[id]
    delete this.s.baselines[id]
    this.save()
  }

  clearedAt(id: PanelId): number {
    return this.s.cleared[id] ?? 0
  }

  baseline(id: PanelId): Record<string, number> {
    return this.s.baselines[id] ?? {}
  }

  reset() {
    this.s = defaults()
    this.save()
  }
}

export const layout = new Layout()
