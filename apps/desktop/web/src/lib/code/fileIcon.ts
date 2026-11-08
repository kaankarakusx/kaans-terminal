// Dosya türü simgeleri (kendi tasarımımız; VS Code'un simge fontu kullanılmaz). Renkler tema token'larından.
export interface FileIcon {
  /** Kısa işaret (1–2 karakter) */
  glyph: string
  /** CSS rengi (token) */
  color: string
}

const BY_EXT: Record<string, FileIcon> = {
  ts: { glyph: 'TS', color: 'var(--blue)' },
  mts: { glyph: 'TS', color: 'var(--blue)' },
  tsx: { glyph: '⚛', color: 'var(--cyan)' },
  jsx: { glyph: '⚛', color: 'var(--cyan)' },
  js: { glyph: 'JS', color: 'var(--amber)' },
  mjs: { glyph: 'JS', color: 'var(--amber)' },
  cjs: { glyph: 'JS', color: 'var(--amber)' },
  json: { glyph: '{}', color: 'var(--amber)' },
  jsonc: { glyph: '{}', color: 'var(--amber)' },
  rs: { glyph: 'R', color: 'var(--accent)' },
  svelte: { glyph: 'S', color: 'var(--red)' },
  vue: { glyph: 'V', color: 'var(--green)' },
  css: { glyph: '#', color: 'var(--blue)' },
  scss: { glyph: '#', color: 'var(--magenta)' },
  html: { glyph: '<>', color: 'var(--accent)' },
  md: { glyph: 'M↓', color: 'var(--sky)' },
  mdx: { glyph: 'M↓', color: 'var(--sky)' },
  txt: { glyph: '≡', color: 'var(--txt3)' },
  py: { glyph: 'Py', color: 'var(--sky)' },
  go: { glyph: 'Go', color: 'var(--cyan)' },
  java: { glyph: 'J', color: 'var(--red)' },
  kt: { glyph: 'K', color: 'var(--violet)' },
  swift: { glyph: 'Sw', color: 'var(--accent)' },
  c: { glyph: 'C', color: 'var(--blue)' },
  h: { glyph: 'H', color: 'var(--violet)' },
  cpp: { glyph: 'C+', color: 'var(--blue)' },
  yaml: { glyph: '!', color: 'var(--red)' },
  yml: { glyph: '!', color: 'var(--red)' },
  toml: { glyph: '⚙', color: 'var(--txt3)' },
  lock: { glyph: '🔒︎', color: 'var(--dim)' },
  sh: { glyph: '$', color: 'var(--green)' },
  zsh: { glyph: '$', color: 'var(--green)' },
  sql: { glyph: 'DB', color: 'var(--amber)' },
  svg: { glyph: '✱', color: 'var(--amber)' },
  png: { glyph: '▣', color: 'var(--violet)' },
  jpg: { glyph: '▣', color: 'var(--violet)' },
  jpeg: { glyph: '▣', color: 'var(--violet)' },
  gif: { glyph: '▣', color: 'var(--violet)' },
  webp: { glyph: '▣', color: 'var(--violet)' },
  ico: { glyph: '▣', color: 'var(--violet)' },
  pdf: { glyph: 'P', color: 'var(--red)' },
  env: { glyph: '⚿', color: 'var(--amber)' },
}

const BY_NAME: Record<string, FileIcon> = {
  'package.json': { glyph: '⬢', color: 'var(--green)' },
  'package-lock.json': { glyph: '⬢', color: 'var(--dim)' },
  'Cargo.toml': { glyph: '⚙', color: 'var(--accent)' },
  'Cargo.lock': { glyph: '🔒︎', color: 'var(--dim)' },
  Dockerfile: { glyph: '◫', color: 'var(--sky)' },
  '.gitignore': { glyph: '◆', color: 'var(--accent)' },
  'tsconfig.json': { glyph: 'TS', color: 'var(--sky)' },
  LICENSE: { glyph: '§', color: 'var(--amber)' },
  'README.md': { glyph: 'ⓘ', color: 'var(--sky)' },
  'CLAUDE.md': { glyph: '✦', color: 'var(--accent)' },
}

const DEFAULT: FileIcon = { glyph: '▢', color: 'var(--txt3)' }

export function fileIcon(path: string): FileIcon {
  const name = path.split('/').at(-1) ?? path
  if (BY_NAME[name]) return BY_NAME[name]
  if (name.startsWith('.env')) return BY_EXT.env
  const ext = name.includes('.') ? name.split('.').at(-1)!.toLowerCase() : ''
  return BY_EXT[ext] ?? DEFAULT
}
