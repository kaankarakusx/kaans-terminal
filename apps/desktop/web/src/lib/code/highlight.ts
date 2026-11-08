// Sözdizimi renklendirme (OZELLIKLER.md §4): Shiki (TextMate gramerleri, MIT). WASM yok (CSP'ye uyar): JavaScript
// düzenli ifade motoru. Renkler CSS değişkenleri (`--shk-*`) → code.css onları uygulamanın temasına bağlar; tema
// değişince yeniden renklendirmek gerekmez. Diller ilk kullanıldıklarında yüklenir.
import { createCssVariablesTheme, createHighlighterCore, type HighlighterCore, type ThemedToken } from 'shiki/core'
import { createJavaScriptRegexEngine } from 'shiki/engine/javascript'

const THEME = createCssVariablesTheme({ name: 'kaans', variablePrefix: '--shk-', variableDefaults: {}, fontStyle: true })

/** Dil → modül (Vite ayrı parçalar üretir; yalnızca açılan dil indirilir… uygulamanın kendi paketinden). */
const LANGS: Record<string, () => Promise<unknown>> = {
  typescript: () => import('shiki/langs/typescript.mjs'),
  tsx: () => import('shiki/langs/tsx.mjs'),
  javascript: () => import('shiki/langs/javascript.mjs'),
  jsx: () => import('shiki/langs/jsx.mjs'),
  json: () => import('shiki/langs/json.mjs'),
  rust: () => import('shiki/langs/rust.mjs'),
  svelte: () => import('shiki/langs/svelte.mjs'),
  css: () => import('shiki/langs/css.mjs'),
  html: () => import('shiki/langs/html.mjs'),
  markdown: () => import('shiki/langs/markdown.mjs'),
  python: () => import('shiki/langs/python.mjs'),
  go: () => import('shiki/langs/go.mjs'),
  toml: () => import('shiki/langs/toml.mjs'),
  yaml: () => import('shiki/langs/yaml.mjs'),
  shellscript: () => import('shiki/langs/shellscript.mjs'),
  sql: () => import('shiki/langs/sql.mjs'),
  java: () => import('shiki/langs/java.mjs'),
  kotlin: () => import('shiki/langs/kotlin.mjs'),
  swift: () => import('shiki/langs/swift.mjs'),
  c: () => import('shiki/langs/c.mjs'),
  cpp: () => import('shiki/langs/cpp.mjs'),
}

const EXT: Record<string, string> = {
  ts: 'typescript', mts: 'typescript', cts: 'typescript', tsx: 'tsx', js: 'javascript', mjs: 'javascript',
  cjs: 'javascript', jsx: 'jsx', json: 'json', jsonc: 'json', rs: 'rust', svelte: 'svelte', css: 'css',
  html: 'html', htm: 'html', md: 'markdown', py: 'python', go: 'go', toml: 'toml', yml: 'yaml', yaml: 'yaml',
  sh: 'shellscript', bash: 'shellscript', zsh: 'shellscript', sql: 'sql', java: 'java', kt: 'kotlin',
  kts: 'kotlin', swift: 'swift', c: 'c', h: 'c', cc: 'cpp', cpp: 'cpp', hpp: 'cpp', cxx: 'cpp',
}

/** Dosya adından dil; bilinmiyorsa null (düz metin). */
export function langOf(path: string): string | null {
  const name = path.split('/').at(-1) ?? ''
  if (name === 'Dockerfile' || name.endsWith('.Dockerfile')) return null
  if (name === 'Cargo.lock') return 'toml'
  const ext = name.includes('.') ? name.split('.').at(-1)!.toLowerCase() : ''
  return EXT[ext] ?? null
}

/** Arayüzde gösterilen dil adı */
export function langLabel(lang: string | null): string {
  return lang ?? 'plain text'
}

let core: Promise<HighlighterCore> | null = null
const loaded = new Set<string>()

function highlighter(): Promise<HighlighterCore> {
  core ??= createHighlighterCore({ themes: [THEME], langs: [], engine: createJavaScriptRegexEngine() })
  return core
}

/** Bu boyutun üstündeki dosyalar renklendirilmez (arayüz donmasın). */
const MAX_HIGHLIGHT = 300_000

export interface Tok {
  content: string
  /** `var(--shk-…)` ya da undefined (düz yazı rengi) */
  color?: string
  italic?: boolean
  bold?: boolean
}

/** Metni satır satır token'lara böler. Dil yoksa ya da çok büyükse düz satırlar. */
export async function tokenize(code: string, lang: string | null): Promise<Tok[][]> {
  const plain = () => code.split('\n').map((l) => [{ content: l }])
  if (!lang || !LANGS[lang] || code.length > MAX_HIGHLIGHT) return plain()
  try {
    const h = await highlighter()
    if (!loaded.has(lang)) {
      const mod = (await LANGS[lang]()) as { default: Parameters<HighlighterCore['loadLanguage']>[0] }
      await h.loadLanguage(mod.default)
      loaded.add(lang)
    }
    const lines: ThemedToken[][] = h.codeToTokensBase(code, { lang, theme: 'kaans' })
    return lines.map((line) =>
      line.map((t) => ({
        content: t.content,
        color: t.color,
        italic: !!(t.fontStyle && t.fontStyle & 1),
        bold: !!(t.fontStyle && t.fontStyle & 2),
      })),
    )
  } catch (e) {
    console.warn('[highlight]', lang, e)
    return plain()
  }
}
