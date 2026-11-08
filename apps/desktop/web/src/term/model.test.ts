import { beforeEach, describe, expect, it } from 'vitest'
import { WindowModel, leaves, readIndex, windowFor } from './model.svelte'

// vitest node ortamında localStorage yok: basit bir bellek içi karşılık.
const store = new Map<string, string>()
globalThis.localStorage = {
  getItem: (k: string) => store.get(k) ?? null,
  setItem: (k: string, v: string) => void store.set(k, v),
  removeItem: (k: string) => void store.delete(k),
  clear: () => store.clear(),
  key: () => null,
  length: 0,
} as Storage

beforeEach(() => store.clear())

const sessionsOf = (m: WindowModel) => m.tab ? leaves(m.tab.root).map((l) => l.session) : []

describe('WindowModel', () => {
  it('adds tabs at the front and splits the active pane', () => {
    const m = new WindowModel('w1')
    m.addTab('s1')
    m.addTab('s2')
    expect(m.tabs.map((t) => leaves(t.root)[0].session)).toEqual(['s2', 's1'])
    m.split('row', 's3')
    expect(sessionsOf(m)).toEqual(['s2', 's3'])
    expect(m.activePane?.session).toBe('s3')
    m.split('col', 's4')
    expect(sessionsOf(m)).toEqual(['s2', 's3', 's4'])
  })

  it('closing a pane promotes its sibling; last pane closes the tab and then the window', () => {
    const m = new WindowModel('w2')
    m.addTab('a')
    m.split('row', 'b')
    const b = m.activePane!.id
    expect(m.closePane(b)).toBe(false)
    expect(m.tab?.root.kind).toBe('leaf')
    expect(m.closePane(m.activePane!.id)).toBe(true)
    expect(m.tabs).toHaveLength(0)
  })

  it('reopens the last closed tab only with sessions still alive', () => {
    const m = new WindowModel('w3')
    m.addTab('dead')
    m.addTab('alive')
    m.split('row', 'gone')
    m.closeTab(0)
    m.reopen((s) => s === 'alive')
    expect(sessionsOf(m)).toEqual(['alive'])
  })

  it('cycles panes and resizes the nearest matching split', () => {
    const m = new WindowModel('w4')
    m.addTab('a')
    m.split('row', 'b')
    m.cyclePane(1)
    expect(m.activePane?.session).toBe('a')
    m.resizeActive('row', 0.1)
    expect(m.tab?.root.kind === 'split' && m.tab.root.ratio).toBeCloseTo(0.6)
    m.resizeActive('col', 0.1) // dikey bölme yok: değişmez
    expect(m.tab?.root.kind === 'split' && m.tab.root.ratio).toBeCloseTo(0.6)
  })

  it('dropping a tab on a pane splits that pane on the chosen side', () => {
    const m = new WindowModel('w5')
    m.addTab('target')
    m.addTab('moving')
    const targetPane = leaves(m.tabs[1].root)[0].id
    m.dropTabOnPane(0, targetPane, 'left')
    expect(m.tabs).toHaveLength(1)
    const root = m.tabs[0].root
    expect(root.kind === 'split' && root.dir).toBe('row')
    expect(leaves(root).map((l) => l.session)).toEqual(['moving', 'target'])
  })

  it('drops dead sessions and persists to the window index', () => {
    const m = new WindowModel('w6')
    m.addTab('x')
    m.addTab('y')
    expect(readIndex()).toContain('w6')
    expect(windowFor('x')).toBe('w6')
    m.dropSession('x')
    expect(m.sessions()).toEqual(['y'])
    const again = new WindowModel('w6')
    expect(again.sessions()).toEqual(['y'])
    expect(m.dropSession('y')).toBe(true)
    expect(readIndex()).not.toContain('w6')
  })

  it('moves tabs within bounds', () => {
    const m = new WindowModel('w7')
    m.addTab('a')
    m.addTab('b')
    m.addTab('c')
    m.moveTab(0, 2)
    expect(m.tabs.map((t) => leaves(t.root)[0].session)).toEqual(['b', 'a', 'c'])
    m.moveTab(0, 5)
    expect(m.tabs).toHaveLength(3)
  })
})
