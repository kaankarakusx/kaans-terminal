// `git diff` (unified) çözümleyici: satır numaralarıyla birlikte. Renklendirme eski/yeni dosyanın tamamından
// satır numarasıyla alınır (çok satırlı yorumlar doğru boyansın).

export interface DiffLine {
  kind: 'ctx' | 'add' | 'del' | 'hunk'
  /** Eski dosyadaki satır (1'den), yoksa null */
  old: number | null
  /** Yeni dosyadaki satır (1'den), yoksa null */
  new: number | null
  /** Yalnızca 'hunk' satırında: @@ başlığının sonundaki bağlam */
  text?: string
}

export function parsePatch(patch: string): DiffLine[] {
  const out: DiffLine[] = []
  let o = 0
  let n = 0
  let inHunk = false
  for (const line of patch.split('\n')) {
    const h = line.match(/^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@ ?(.*)$/)
    if (h) {
      o = Number(h[1])
      n = Number(h[2])
      inHunk = true
      out.push({ kind: 'hunk', old: null, new: null, text: h[3] })
      continue
    }
    if (!inHunk) continue
    if (line.startsWith('+')) out.push({ kind: 'add', old: null, new: n++ })
    else if (line.startsWith('-')) out.push({ kind: 'del', old: o++, new: null })
    else if (line.startsWith(' ')) out.push({ kind: 'ctx', old: o++, new: n++ })
    else if (line.startsWith('\\')) continue // "\ No newline at end of file"
    else if (line.startsWith('diff ')) inHunk = false
  }
  return out
}

/** İzlenmeyen/yeni dosya: hepsi eklenmiş. */
export function allAdded(lineCount: number): DiffLine[] {
  const out: DiffLine[] = [{ kind: 'hunk', old: null, new: null, text: 'new file' }]
  for (let i = 1; i <= lineCount; i++) out.push({ kind: 'add', old: null, new: i })
  return out
}

/** Silinmiş dosya: hepsi silinmiş. */
export function allDeleted(lineCount: number): DiffLine[] {
  const out: DiffLine[] = [{ kind: 'hunk', old: null, new: null, text: 'deleted file' }]
  for (let i = 1; i <= lineCount; i++) out.push({ kind: 'del', old: i, new: null })
  return out
}

/** Satır sayısı (sondaki boş satır sayılmaz) */
export function lineCount(text: string): number {
  if (!text) return 0
  return text.endsWith('\n') ? text.split('\n').length - 1 : text.split('\n').length
}

/** #rgb / #rrggbb / #rrggbbaa renk örnekleri */
export const COLOR_RE = /#(?:[0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{3})\b/g

/**
 * Sticky scroll: `top` satırını (0'dan) kapsayan başlık satırları (girintisi küçük, blok açan satırlar), en dıştan
 * içe. En fazla `max`.
 */
export function stickyLines(lines: string[], top: number, max = 3): number[] {
  const indent = (s: string) => s.match(/^\s*/)![0].replace(/\t/g, '    ').length
  const opens = (s: string) => /[{([:]\s*(\/\/.*)?$/.test(s.trimEnd()) || /^\s*(export\s+)?(async\s+)?(fn|function|class|def|impl|struct|enum|trait|mod|interface)\b/.test(s)
  const out: number[] = []
  if (top <= 0 || top >= lines.length) return out
  let cur = indent(lines[top])
  // boş satırda bir sonraki dolu satırın girintisi
  if (!lines[top].trim()) {
    for (let i = top; i < lines.length; i++) if (lines[i].trim()) { cur = indent(lines[i]); break }
  }
  for (let i = top - 1; i >= 0 && cur > 0; i--) {
    const l = lines[i]
    if (!l.trim()) continue
    const d = indent(l)
    if (d < cur && opens(l)) {
      out.unshift(i)
      cur = d
    }
  }
  return out.slice(-max)
}
