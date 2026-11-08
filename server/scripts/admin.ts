// Yönetici aracı: duyuru yaz, geri bildirimi oku.
//   ADMIN_TOKEN=… node scripts/admin.ts announce "Başlık" "Metin (bağlantılar ve @github-adı olabilir)"
//   ADMIN_TOKEN=… node scripts/admin.ts feedback
// Sunucu: SERVER (varsayılan https://kaansterminal.karakuslabs.com)
const server = (process.env.SERVER ?? 'https://kaansterminal.karakuslabs.com').replace(/\/$/, '')
const token = process.env.ADMIN_TOKEN
if (!token) throw new Error('ADMIN_TOKEN gerekli')
const H = { authorization: `Bearer ${token}`, 'content-type': 'application/json' }
const [cmd, title, body] = process.argv.slice(2)

if (cmd === 'announce') {
  if (!title || !body) throw new Error('kullanım: announce "başlık" "metin"')
  const r = await fetch(`${server}/api/v1/admin/announcements`, { method: 'POST', headers: H, body: JSON.stringify({ title, body }) })
  console.log(r.status, await r.text())
} else if (cmd === 'feedback') {
  const r = await fetch(`${server}/api/v1/admin/feedback`, { headers: H })
  const { items } = (await r.json()) as { items: Array<{ name_tag: string; body: string; app_version: string; created_at: number }> }
  for (const f of items) console.log(`${new Date(f.created_at).toISOString()}  ${f.name_tag}  v${f.app_version ?? '?'}\n  ${f.body}\n`)
} else {
  console.log('komutlar: announce "başlık" "metin" | feedback')
}
