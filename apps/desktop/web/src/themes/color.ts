// sRGB ↔ OKLab/OKLCH dönüşümleri (Björn Ottosson). Karıştırma OKLCH'de yapılır; açıklık göze eşit görünür.

export interface Oklch {
  l: number
  c: number
  /** derece; renksizde (c≈0) anlamsız */
  h: number
}

const toLinear = (v: number) => {
  const c = v / 255
  return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4
}
const toSrgb = (v: number) => {
  const c = v <= 0.0031308 ? 12.92 * v : 1.055 * Math.cbrt(v) ** (3 / 2.4) - 0.055
  return Math.round(Math.min(1, Math.max(0, c)) * 255)
}

export function hexToRgb(hex: string): [number, number, number] {
  const h = hex.replace('#', '')
  const full = h.length === 3 ? [...h].map((x) => x + x).join('') : h
  if (!/^[0-9a-fA-F]{6}$/.test(full)) throw new Error(`geçersiz renk: ${hex}`)
  return [0, 2, 4].map((i) => parseInt(full.slice(i, i + 2), 16)) as [number, number, number]
}

export function rgbToHex([r, g, b]: [number, number, number]): string {
  return '#' + [r, g, b].map((v) => v.toString(16).padStart(2, '0')).join('')
}

export function toOklch(hex: string): Oklch {
  const [r, g, b] = hexToRgb(hex).map(toLinear)
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b)
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b)
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b)
  const L = 0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s
  const A = 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s
  const B = 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s
  const h = (Math.atan2(B, A) * 180) / Math.PI
  return { l: L, c: Math.hypot(A, B), h: h < 0 ? h + 360 : h }
}

export function fromOklch({ l: L, c, h }: Oklch): string {
  const A = c * Math.cos((h * Math.PI) / 180)
  const B = c * Math.sin((h * Math.PI) / 180)
  const l = (L + 0.3963377774 * A + 0.2158037573 * B) ** 3
  const m = (L - 0.1055613458 * A - 0.0638541728 * B) ** 3
  const s = (L - 0.0894841775 * A - 1.291485548 * B) ** 3
  return rgbToHex([
    toSrgb(4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s),
    toSrgb(-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s),
    toSrgb(-0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s),
  ])
}

const ACHROMATIC = 0.02

/** a'dan b'ye t kadar (0 → a, 1 → b), OKLCH'de, tonda kısa yoldan. */
export function mix(a: string, b: string, t: number): string {
  const x = toOklch(a)
  const y = toOklch(b)
  // Renksiz tarafın tonu yoktur; diğerininkini kullan (beyaza karıştırınca ton kaymasın).
  const hx = x.c < ACHROMATIC ? y.h : x.h
  const hy = y.c < ACHROMATIC ? x.h : y.h
  let dh = hy - hx
  if (dh > 180) dh -= 360
  if (dh < -180) dh += 360
  return fromOklch({ l: x.l + (y.l - x.l) * t, c: x.c + (y.c - x.c) * t, h: (hx + dh * t + 360) % 360 })
}

/** Açıklığı çarpanla değiştirir (0.8 → %20 koyu). */
export function scaleLightness(hex: string, k: number): string {
  const c = toOklch(hex)
  return fromOklch({ ...c, l: Math.min(1, c.l * k) })
}

/** İki rengin ton farkı (derece, 0–180). */
export function hueDistance(a: string, b: string): number {
  const d = Math.abs(toOklch(a).h - toOklch(b).h) % 360
  return d > 180 ? 360 - d : d
}

export function rgbTriplet(hex: string): string {
  return hexToRgb(hex).join(', ')
}
