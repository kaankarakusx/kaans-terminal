// Türetilmiş token'ları sayfaya yazar. Sayfa yeniden yüklenmez; JS ile çizenler `themechange` olayını dinler.
import defaultTheme from './default.json'
import { deriveTheme, type ThemeFile, type Tokens } from './derive'

export const DEFAULT_THEME = defaultTheme as ThemeFile

export function applyTokens(tokens: Tokens, root: HTMLElement = document.documentElement) {
  for (const [k, v] of Object.entries(tokens)) root.style.setProperty(k, v)
  window.dispatchEvent(new CustomEvent('themechange', { detail: tokens }))
}

export function applyTheme(theme: ThemeFile = DEFAULT_THEME) {
  applyTokens(deriveTheme(theme))
}

declare global {
  interface Window {
    /** Rust → sayfa köprüsü (MIMARI.md §5): tema değişince çağrılır. */
    __theme?: (theme: ThemeFile) => void
  }
}
