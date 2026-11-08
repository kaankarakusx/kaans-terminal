// Seçili tema, önizleme ve INTERFACE ayarları (yazı tipi, boyut). localStorage'da; bütün pencereler aynı
// kökenden olduğu için `storage` olayıyla birbirini anında günceller (ana panel + terminal pencereleri).
import { applyTokens } from './apply'
import { deriveTheme } from './derive'
import { THEMES, themeById } from './index'

const K_THEME = 'theme.id'
const K_PREVIEW = 'theme.preview'
const K_FONT = 'ui.font'
const K_SCALE = 'ui.scale'

export type UiFont = 'mono' | 'system' | 'rounded'
export const FONTS: Record<UiFont, string> = {
  mono: '"SF Mono", "JetBrains Mono", Menlo, Monaco, Consolas, monospace',
  system: '-apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif',
  rounded: 'ui-rounded, "SF Pro Rounded", "Arial Rounded MT Bold", system-ui, sans-serif',
}
export const SCALE_MIN = 1
export const SCALE_MAX = 1.4

function read(k: string): string | null {
  try {
    return localStorage.getItem(k)
  } catch {
    return null
  }
}
function write(k: string, v: string | null) {
  try {
    if (v == null) localStorage.removeItem(k)
    else localStorage.setItem(k, v)
  } catch {
    /* depolama kapalı: yalnızca bu pencerede geçerli */
  }
}

const clampScale = (v: number) => (Number.isFinite(v) ? Math.min(SCALE_MAX, Math.max(SCALE_MIN, v)) : 1)

class ThemeStore {
  id = $state(themeById(read(K_THEME)).id)
  preview = $state<string | null>(null)
  font = $state<UiFont>((read(K_FONT) as UiFont) in FONTS ? (read(K_FONT) as UiFont) : 'mono')
  scale = $state(clampScale(Number(read(K_SCALE) ?? 1)))

  readonly all = THEMES
  /** Şu an ekranda görünen tema (önizleme varsa o) */
  get shown() {
    return themeById(this.preview ?? this.id)
  }

  /** Bileşenler bağlanmadan önce bir kez: tema + arayüz ayarları + diğer pencereleri dinleme. */
  init() {
    this.paint()
    this.paintInterface()
    window.addEventListener('storage', (e) => {
      if (e.key === K_THEME) this.id = themeById(e.newValue).id
      else if (e.key === K_PREVIEW) this.preview = e.newValue
      else if (e.key === K_FONT && e.newValue && e.newValue in FONTS) this.font = e.newValue as UiFont
      else if (e.key === K_SCALE) this.scale = clampScale(Number(e.newValue ?? 1))
      else return
      if (e.key === K_THEME || e.key === K_PREVIEW) this.paint()
      else this.paintInterface()
    })
  }

  select(id: string) {
    this.id = themeById(id).id
    this.preview = null
    write(K_THEME, this.id)
    write(K_PREVIEW, null)
    this.paint()
  }

  /** Üzerine gelince bütün uygulama o temada görünür; null önizlemeyi bitirir. */
  setPreview(id: string | null) {
    if (id === this.preview) return
    this.preview = id === this.id ? null : id
    write(K_PREVIEW, this.preview)
    this.paint()
  }

  setFont(f: UiFont) {
    this.font = f
    write(K_FONT, f)
    this.paintInterface()
  }

  setScale(v: number) {
    this.scale = clampScale(v)
    write(K_SCALE, String(this.scale))
    this.paintInterface()
  }

  private paint() {
    const t = this.shown
    applyTokens(deriveTheme(t))
    document.documentElement.classList.toggle('sig', !!t.signature)
  }

  private paintInterface() {
    const root = document.documentElement.style
    root.setProperty('--font-ui', FONTS[this.font])
    root.setProperty('--fs-scale', String(this.scale))
  }
}

export const themes = new ThemeStore()
