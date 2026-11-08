import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'
import { fromOklch, hueDistance, mix, toOklch } from './color'
import defaultTheme from './default.json'
import { deriveTheme, type ThemeFile } from './derive'

const HEX = /^#[0-9a-f]{6}$/
const RGB = /^\d{1,3}, \d{1,3}, \d{1,3}$/

function cssTokens(): Record<string, string> {
  const css = readFileSync(new URL('./tokens.css', import.meta.url), 'utf8')
  const out: Record<string, string> = {}
  for (const m of css.matchAll(/(--[\w-]+):\s*([^;]+);/g)) out[m[1]] = m[2].trim()
  return out
}

describe('color', () => {
  it('round-trips through OKLCH', () => {
    for (const c of ['#f88008', '#00e5ff', '#0a0705', '#ffffff', '#000000', '#9d6bff']) {
      expect(fromOklch(toOklch(c))).toBe(c)
    }
  })
  it('mixes endpoints exactly and keeps hue when mixing with white', () => {
    expect(mix('#f88008', '#ffffff', 0)).toBe('#f88008')
    expect(mix('#f88008', '#ffffff', 1)).toBe('#ffffff')
    const half = mix('#f88008', '#ffffff', 0.5)
    expect(hueDistance(half, '#f88008')).toBeLessThan(5)
  })
})

describe('deriveTheme', () => {
  it('default theme matches tokens.css (no flash of other colors at startup)', () => {
    const derived = deriveTheme(defaultTheme as ThemeFile)
    const css = cssTokens()
    const colorKeys = Object.keys(derived)
    expect(colorKeys.length).toBeGreaterThan(20)
    for (const k of colorKeys) {
      expect(css[k], `${k} tokens.css içinde yok`).toBeDefined()
      expect(css[k].toLowerCase(), k).toBe(derived[k].toLowerCase())
    }
  })

  const dracula: ThemeFile = {
    id: 'dracula',
    name: 'Dracula',
    bg: '#282a36',
    fg: '#f8f8f2',
    cursor: '#f8f8f2',
    selection: '#44475a',
    accent: '#bd93f9',
    ansi: [
      '#21222c', '#ff5555', '#50fa7b', '#f1fa8c', '#bd93f9', '#ff79c6', '#8be9fd', '#f8f8f2',
      '#6272a4', '#ff6e6e', '#69ff94', '#ffffa5', '#d6acff', '#ff92df', '#a4ffff', '#ffffff',
    ],
  }

  it('derives every token as valid colors with matching rgb pairs', () => {
    const t = deriveTheme(dracula)
    for (const [k, v] of Object.entries(t)) {
      expect(v, k).toMatch(k.endsWith('-rgb') ? RGB : HEX)
    }
    expect(t['--red']).toBe('#ff6e6e')
    expect(t['--cyan']).toBe('#a4ffff')
    expect(t['--accent-rgb']).toBe('189, 147, 249')
    // ikincil yazılar yazıdan zemine doğru sıralı koyulaşır
    const L = (k: string) => toOklch(t[k]).l
    expect(L('--hi')).toBeGreaterThanOrEqual(L('--txt'))
    expect(L('--txt')).toBeGreaterThan(L('--txt2'))
    expect(L('--txt2')).toBeGreaterThan(L('--dim'))
    expect(L('--dim')).toBeGreaterThan(L('--dim2'))
    expect(L('--dim2')).toBeGreaterThan(L('--bg'))
  })

  it('moves "your turn" to green when the accent is cyan-like', () => {
    const nord = { ...dracula, id: 'nord', accent: '#88c0d0', ansi: [...dracula.ansi] }
    nord.ansi[14] = '#8fbcbb'
    const t = deriveTheme(nord)
    expect(t['--cyan']).toBe(t['--green'])
  })

  it('rejects malformed palettes', () => {
    expect(() => deriveTheme({ ...dracula, ansi: dracula.ansi.slice(0, 8) })).toThrow()
  })
})

describe('theme list', () => {
  it('every theme derives valid, distinct tokens', async () => {
    const { THEMES } = await import('./index')
    expect(THEMES.length).toBe(14)
    expect(new Set(THEMES.map((t) => t.id)).size).toBe(THEMES.length)
    for (const t of THEMES) {
      for (const c of [t.bg, t.fg, t.cursor, t.selection, t.accent, ...t.ansi]) expect(c, `${t.id}`).toMatch(HEX)
      const d = deriveTheme(t)
      for (const [k, v] of Object.entries(d)) {
        if (k.endsWith('-rgb')) expect(v, `${t.id} ${k}`).toMatch(RGB)
        else expect(v, `${t.id} ${k}`).toMatch(HEX)
      }
      // "your turn" vurgudan ayırt edilebilir kalmalı
      expect(hueDistance(d['--accent'], d['--cyan']), t.id).toBeGreaterThanOrEqual(30)
    }
  })
})
