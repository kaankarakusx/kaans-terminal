// Duyuru/mesaj metnini parçalara böler: http(s) bağlantıları ve @ad (GitHub profili) tıklanabilir.
// HTML üretmez (XSS yok): bileşen parçaları metin olarak çizer.
export type Seg = { text: string; href?: string }

const RE = /(https?:\/\/[^\s<>"')\]]+)|(^|[^\w@])@([A-Za-z0-9](?:[A-Za-z0-9-]{0,38}))\b/g

export function richText(s: string): Seg[] {
  const out: Seg[] = []
  let last = 0
  for (const m of s.matchAll(RE)) {
    const start = m.index!
    if (m[1]) {
      // Cümle sonundaki noktalama bağlantıya dahil değil
      const url = m[1].replace(/[.,;:!?]+$/, '')
      if (start > last) out.push({ text: s.slice(last, start) })
      out.push({ text: url, href: url })
      last = start + url.length
    } else {
      const at = start + m[2].length
      if (at > last) out.push({ text: s.slice(last, at) })
      out.push({ text: `@${m[3]}`, href: `https://github.com/${m[3]}` })
      last = at + 1 + m[3].length
    }
  }
  if (last < s.length) out.push({ text: s.slice(last) })
  return out
}
