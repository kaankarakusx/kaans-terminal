// xterm renkleri uygulamanın temasından (TASARIM.md §9.3): arka plan, yazı, imleç, seçim ve 16 ANSI rengi.
import type { ITheme } from '@xterm/xterm'
import { DEFAULT_THEME } from '../themes/apply'

const css = (name: string, fallback: string) =>
  getComputedStyle(document.documentElement).getPropertyValue(name).trim() || fallback

/** Temanın 16 ANSI rengi (`--ansi-0` … `--ansi-15`) */
export function currentAnsi(): string[] {
  return DEFAULT_THEME.ansi.map((c, i) => css(`--ansi-${i}`, c))
}

export function xtermTheme(transparent = false, ansi: string[] = currentAnsi()): ITheme {
  const [black, red, green, yellow, blue, magenta, cyan, white, bBlack, bRed, bGreen, bYellow, bBlue, bMagenta, bCyan, bWhite] = ansi
  return {
    background: transparent ? 'rgba(0, 0, 0, 0)' : css('--term-bg', DEFAULT_THEME.bg),
    foreground: css('--txt', DEFAULT_THEME.fg),
    cursor: css('--accent', DEFAULT_THEME.cursor),
    cursorAccent: css('--term-bg', DEFAULT_THEME.bg),
    selectionBackground: css('--selection', DEFAULT_THEME.selection),
    black, red, green, yellow, blue, magenta, cyan, white,
    brightBlack: bBlack, brightRed: bRed, brightGreen: bGreen, brightYellow: bYellow,
    brightBlue: bBlue, brightMagenta: bMagenta, brightCyan: bCyan, brightWhite: bWhite,
  }
}

export const TERM_FONT = '"SF Mono", "JetBrains Mono", Menlo, Monaco, Consolas, monospace'
