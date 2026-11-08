// Tema dosyasından arayüz token'larını türetir (TASARIM.md §9.2). Tek yer: hem sayfa hem ileride native kod
// bu düz sözlüğü kullanır.
import { hueDistance, mix, rgbTriplet, scaleLightness } from './color'

export interface ThemeFile {
  id: string
  name: string
  bg: string
  fg: string
  cursor: string
  selection: string
  accent: string
  /** 16 ANSI rengi: 0–7 normal, 8–15 parlak */
  ansi: string[]
  /** İmza tema: ek görsel katman (`html.sig`, signature.css) */
  signature?: boolean
  /** Elle düzeltmeler; türetilen değerlerin üzerine yazar. */
  ui?: Record<string, string>
}

export type Tokens = Record<string, string>

const WHITE = '#ffffff'
const BLACK = '#000000'
/** "your turn" camgöbeği vurguya bu kadar yakınsa yeşile geçer. */
const MIN_STATUS_HUE_GAP = 30

/** `-rgb` eşi olan token'lar: rgba(var(--x-rgb), .3) için. */
const RGB_PAIRS = ['--bg', '--term-bg', '--accent', '--accent-soft', '--cyan', '--warn', '--violet', '--amber', '--red', '--green']

export function deriveTheme(t: ThemeFile): Tokens {
  if (t.ansi.length !== 16) throw new Error(`${t.id}: ansi 16 renk olmalı`)
  const a = t.ansi
  const bg = scaleLightness(t.bg, 0.6)
  const panel = scaleLightness(t.bg, 0.8)

  const out: Tokens = {
    '--bg': bg,
    '--panel-rgb': rgbTriplet(panel),
    '--term-bg': t.bg,
    '--accent': t.accent,
    '--accent-soft': mix(t.accent, WHITE, 0.35),
    '--accent-pale': mix(t.accent, WHITE, 0.65),
    '--ink': mix(t.bg, BLACK, 0.4),
    '--txt': t.fg,
    '--hi': mix(t.fg, WHITE, 0.5),
    '--txt2': mix(t.fg, bg, 0.15),
    '--txt3': mix(t.fg, bg, 0.3),
    '--dim': mix(t.fg, bg, 0.45),
    '--faint': mix(t.fg, bg, 0.58),
    '--dim2': mix(t.fg, bg, 0.72),
    // anlam renkleri ANSI'den
    '--red': a[9],
    '--green': a[10],
    '--amber': a[11],
    '--blue': a[12],
    '--violet': a[13],
    '--cyan': a[14],
    '--warn': mix(a[9], a[13], 0.2),
    '--sky': mix(a[12], a[14], 0.5),
    '--magenta': mix(a[13], a[9], 0.35),
    '--lime': mix(a[10], a[11], 0.5),
    '--ended': mix(t.fg, bg, 0.68),
  }
  // Terminal renkleri (xterm ve ileride kod görüntüleyici)
  a.forEach((c, i) => (out[`--ansi-${i}`] = c))
  out['--selection'] = t.selection
  Object.assign(out, t.ui ?? {})

  // "your turn" vurgudan ayırt edilmeli (Nord gibi temalarda ikisi de camgöbeği).
  if (hueDistance(out['--accent'], out['--cyan']) < MIN_STATUS_HUE_GAP) out['--cyan'] = out['--green']

  for (const k of RGB_PAIRS) out[`${k}-rgb`] = rgbTriplet(out[k])
  return out
}
