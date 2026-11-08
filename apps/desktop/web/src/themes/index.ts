// Tema listesi (TASARIM.md §9.4). Sıra tema ekranındaki sıradır; ilki varsayılan.
import type { ThemeFile } from './derive'
import { DEFAULT_THEME } from './apply'

const files = import.meta.glob<{ default: ThemeFile }>('./palettes/*.json', { eager: true })
const byId = new Map(Object.values(files).map((m) => [m.default.id, m.default]))

const ORDER = [
  'github-dark', 'one-dark', 'dracula', 'monokai', 'ayu-dark', 'night-owl', 'tokyo-night',
  'catppuccin-mocha', 'gruvbox-dark', 'nord', 'solarized-dark', 'cyberpunk', 'signature',
]

export const THEMES: ThemeFile[] = [
  DEFAULT_THEME,
  ...ORDER.map((id) => byId.get(id)).filter((t): t is ThemeFile => !!t),
  // Listede adı olmayan yeni dosyalar sona
  ...[...byId.values()].filter((t) => !ORDER.includes(t.id)),
]

export function themeById(id: string | null | undefined): ThemeFile {
  return THEMES.find((t) => t.id === id) ?? DEFAULT_THEME
}
