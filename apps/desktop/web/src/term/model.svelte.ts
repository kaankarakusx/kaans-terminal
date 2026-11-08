// Bir terminal penceresinin sekme ve bölme durumu (OZELLIKLER.md §3.3). Kalıcıdır: uygulama yeniden açılınca
// pencereler, oturumları hâlâ yaşıyorsa geri gelir. Kapanan sekme/bölmenin oturumu yaşamaya devam eder.

export type Pane =
  | { kind: 'leaf'; id: string; session: string }
  | { kind: 'split'; id: string; dir: 'row' | 'col'; ratio: number; a: Pane; b: Pane }

export interface Tab {
  id: string
  root: Pane
  active: string
}

export type Side = 'left' | 'right' | 'top' | 'bottom'

const INDEX_KEY = 'term.windows'
const winKey = (label: string) => `term.win.${label}`

let uid = 0
const newId = (p: string) => `${p}${Date.now().toString(36)}${(uid++).toString(36)}`

export function leaves(p: Pane): Extract<Pane, { kind: 'leaf' }>[] {
  return p.kind === 'leaf' ? [p] : [...leaves(p.a), ...leaves(p.b)]
}

function replace(p: Pane, id: string, next: Pane | null): Pane | null {
  if (p.id === id) return next
  if (p.kind === 'leaf') return p
  const a = replace(p.a, id, next)
  const b = replace(p.b, id, next)
  if (!a) return b
  if (!b) return a
  return { ...p, a, b }
}

function find(p: Pane, id: string): Pane | null {
  if (p.id === id) return p
  if (p.kind === 'leaf') return null
  return find(p.a, id) ?? find(p.b, id)
}

/** `id`'ye giden yoldaki bölmeler (kökten yaprağa), hangi tarafta olduğuyla. */
type Step = { split: Extract<Pane, { kind: 'split' }>; side: 'a' | 'b' }

function path(p: Pane, id: string, acc: Step[] = []): Step[] | null {
  if (p.id === id) return acc
  if (p.kind === 'leaf') return null
  return path(p.a, id, [...acc, { split: p, side: 'a' }]) ?? path(p.b, id, [...acc, { split: p, side: 'b' }])
}

export function readIndex(): string[] {
  try {
    return JSON.parse(localStorage.getItem(INDEX_KEY) ?? '[]') as string[]
  } catch {
    return []
  }
}

export function readWindow(label: string): { tabs: Tab[]; activeTab: number } | null {
  try {
    return JSON.parse(localStorage.getItem(winKey(label)) ?? 'null')
  } catch {
    return null
  }
}

/** Bir oturumu gösteren pencere (varsa). */
export function windowFor(session: string): string | null {
  for (const label of readIndex()) {
    const w = readWindow(label)
    if (w?.tabs.some((t) => leaves(t.root).some((l) => l.session === session))) return label
  }
  return null
}

export class WindowModel {
  tabs = $state<Tab[]>([])
  activeTab = $state(0)
  /** ⇧⌘T için kapanan sekmeler (oturumları yaşıyorsa geri gelir) */
  private closed: Tab[] = []

  tab = $derived(this.tabs[this.activeTab] as Tab | undefined)
  activePane = $derived(this.tab ? (find(this.tab.root, this.tab.active) as Extract<Pane, { kind: 'leaf' }> | null) : null)

  constructor(public label: string) {
    const saved = readWindow(label)
    if (saved) {
      this.tabs = saved.tabs
      this.activeTab = Math.min(saved.activeTab, Math.max(0, saved.tabs.length - 1))
    }
  }

  save() {
    try {
      if (this.tabs.length) {
        localStorage.setItem(winKey(this.label), JSON.stringify({ tabs: this.tabs, activeTab: this.activeTab }))
        const idx = readIndex()
        if (!idx.includes(this.label)) localStorage.setItem(INDEX_KEY, JSON.stringify([...idx, this.label]))
      } else {
        localStorage.removeItem(winKey(this.label))
        localStorage.setItem(INDEX_KEY, JSON.stringify(readIndex().filter((l) => l !== this.label)))
      }
    } catch {
      /* depolama yoksa durum yalnızca bu pencerede */
    }
  }

  sessions(): string[] {
    return this.tabs.flatMap((t) => leaves(t.root).map((l) => l.session))
  }

  has(session: string) {
    return this.sessions().includes(session)
  }

  /** Oturumu gösteren sekmeyi ve bölmeyi öne alır. */
  reveal(session: string): boolean {
    for (const [i, t] of this.tabs.entries()) {
      const leaf = leaves(t.root).find((l) => l.session === session)
      if (leaf) {
        this.activeTab = i
        t.active = leaf.id
        this.save()
        return true
      }
    }
    return false
  }

  addTab(session: string) {
    const leaf: Pane = { kind: 'leaf', id: newId('p'), session }
    // Yeni sekmeler en üstten (soldan) başlar.
    this.tabs.unshift({ id: newId('t'), root: leaf, active: leaf.id })
    this.activeTab = 0
    this.save()
  }

  /** Öndeki bölmeyi böler: 'row' yan yana (⌘D), 'col' alt alta (⇧⌘D). */
  split(dir: 'row' | 'col', session: string) {
    const t = this.tab
    if (!t) return this.addTab(session)
    const cur = find(t.root, t.active)
    if (!cur) return
    const leaf: Pane = { kind: 'leaf', id: newId('p'), session }
    const split: Pane = { kind: 'split', id: newId('s'), dir, ratio: 0.5, a: cur, b: leaf }
    t.root = replace(t.root, cur.id, split) ?? leaf
    t.active = leaf.id
    this.save()
  }

  /** Bölmeyi kapatır (oturum yaşar). Döner: pencere boşaldı mı. */
  closePane(paneId: string): boolean {
    const i = this.tabs.findIndex((t) => find(t.root, paneId))
    if (i < 0) return this.tabs.length === 0
    const t = this.tabs[i]
    const rest = replace(t.root, paneId, null)
    if (!rest) {
      this.closed.push(t)
      this.tabs.splice(i, 1)
      this.activeTab = Math.max(0, Math.min(this.activeTab, this.tabs.length - 1))
    } else {
      t.root = rest
      if (t.active === paneId) t.active = leaves(rest)[0].id
    }
    this.save()
    return this.tabs.length === 0
  }

  closeTab(i = this.activeTab): boolean {
    const t = this.tabs[i]
    if (!t) return this.tabs.length === 0
    this.closed.push(t)
    this.tabs.splice(i, 1)
    this.activeTab = Math.max(0, Math.min(this.activeTab, this.tabs.length - 1))
    this.save()
    return this.tabs.length === 0
  }

  /** ⇧⌘T: son kapanan sekme, oturumları hâlâ yaşıyorsa. */
  reopen(alive: (session: string) => boolean) {
    while (this.closed.length) {
      const t = this.closed.pop()!
      if (leaves(t.root).some((l) => alive(l.session))) {
        let root: Pane | null = t.root
        for (const l of leaves(t.root)) if (!alive(l.session) && root) root = replace(root, l.id, null)
        if (!root) continue
        this.tabs.unshift({ ...t, root, active: leaves(root)[0].id })
        this.activeTab = 0
        this.save()
        return
      }
    }
  }

  /** Ölen oturumun bölmelerini kaldırır. Döner: pencere boşaldı mı. */
  dropSession(session: string): boolean {
    for (const t of [...this.tabs]) {
      for (const l of leaves(t.root)) if (l.session === session) this.closePane(l.id)
    }
    return this.tabs.length === 0
  }

  selectTab(i: number) {
    if (i >= 0 && i < this.tabs.length) {
      this.activeTab = i
      this.save()
    }
  }

  /** ⌥⇧⌘[ / ] öndeki sekmeyi taşır; sekme çubuğunda sürükleme de bunu kullanır. */
  moveTab(from: number, to: number) {
    if (from === to || from < 0 || to < 0 || from >= this.tabs.length || to >= this.tabs.length) return
    const [t] = this.tabs.splice(from, 1)
    this.tabs.splice(to, 0, t)
    this.activeTab = to
    this.save()
  }

  focusPane(paneId: string) {
    const i = this.tabs.findIndex((t) => find(t.root, paneId))
    if (i < 0) return
    this.activeTab = i
    this.tabs[i].active = paneId
    this.save()
  }

  /** ⌘] / ⌘[ bölmeler arası. */
  cyclePane(step: 1 | -1) {
    const t = this.tab
    if (!t) return
    const ls = leaves(t.root)
    const i = ls.findIndex((l) => l.id === t.active)
    t.active = ls[(i + step + ls.length) % ls.length].id
    this.save()
  }

  /** ⌃⌘ + ok: öndeki bölmeyi o yönde büyütür. */
  resizeActive(dir: 'row' | 'col', delta: number) {
    const t = this.tab
    if (!t) return
    const route = path(t.root, t.active) ?? []
    for (let k = route.length - 1; k >= 0; k--) {
      const { split, side } = route[k]
      if (split.dir === dir) {
        const target = find(t.root, split.id) as Extract<Pane, { kind: 'split' }>
        target.ratio = Math.min(0.9, Math.max(0.1, target.ratio + (side === 'a' ? delta : -delta)))
        this.save()
        return
      }
    }
  }

  setRatio(splitId: string, ratio: number) {
    const t = this.tab
    const s = t && (find(t.root, splitId) as Extract<Pane, { kind: 'split' }> | null)
    if (s) s.ratio = Math.min(0.9, Math.max(0.1, ratio))
  }

  /** Sekmeyi bir bölmenin üstüne bırak: o bölme ilgili yarıdan bölünür ve sekme ona katılır. */
  dropTabOnPane(tabIndex: number, paneId: string, side: Side) {
    const src = this.tabs[tabIndex]
    const dstIndex = this.tabs.findIndex((t) => find(t.root, paneId))
    if (!src || dstIndex < 0 || dstIndex === tabIndex) return
    const dst = this.tabs[dstIndex]
    const target = find(dst.root, paneId)
    if (!target) return
    const dir = side === 'left' || side === 'right' ? 'row' : 'col'
    const first = side === 'left' || side === 'top'
    const split: Pane = {
      kind: 'split', id: newId('s'), dir, ratio: 0.5,
      a: first ? src.root : target,
      b: first ? target : src.root,
    }
    dst.root = replace(dst.root, target.id, split) ?? split
    dst.active = leaves(src.root)[0].id
    this.tabs.splice(tabIndex, 1)
    this.activeTab = this.tabs.indexOf(dst)
    this.save()
  }
}
