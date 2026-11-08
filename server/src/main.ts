// Sunucuyu başlatır. Ortam değişkenleri:
//   DATABASE_URL  Postgres adresi (yoksa ./data/dev-db içinde PGlite: yalnızca yerel geliştirme)
//   PORT (8787), HOST (127.0.0.1; Docker'da 0.0.0.0), TRUST_PROXY=1 (Cloudflare/nginx arkasında)
//   ADMIN_TOKEN   duyuru yazmak / geri bildirimi okumak için (yoksa yönetici uçları kapalı; en az 32 karakter)
import { buildApp } from './app.ts'
import { migrate, pglite, postgres } from './db.ts'

const url = process.env.DATABASE_URL
const db = url ? postgres(url) : await pglite('./data/dev-db')
if (!url) console.warn('[server] DATABASE_URL yok: yerel PGlite (./data/dev-db) kullanılıyor')
await migrate(db)

const adminToken = process.env.ADMIN_TOKEN
if (adminToken && adminToken.length < 32) throw new Error('ADMIN_TOKEN en az 32 karakter olmalı')
const app = buildApp({ db, trustProxy: process.env.TRUST_PROXY === '1', logger: true, adminToken })
await app.listen({ port: Number(process.env.PORT ?? 8787), host: process.env.HOST ?? '127.0.0.1' })

for (const sig of ['SIGINT', 'SIGTERM'] as const) {
  process.on(sig, async () => {
    await app.close()
    await db.close()
    process.exit(0)
  })
}
