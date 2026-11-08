// Geliştirici kanalı (OZELLIKLER.md §6.5): duyurular ve geri bildirim.
// Duyuruyu yalnızca yönetici yazar: `ADMIN_TOKEN` ortam değişkeni verilmezse yönetici uçları tamamen kapalıdır.
// Geri bildirim name#tag ve zamanla saklanır (e-posta yok).
import { timingSafeEqual } from 'node:crypto'
import type { FastifyInstance, FastifyRequest } from 'fastify'
import type { Db } from './db.ts'
import type { Me } from './friends.ts'
import type { Live } from './live.ts'
import { HttpError, sha256 } from './util.ts'

export interface NewsCtx {
  db: Db
  now: () => number
  live: Live
  device: (req: FastifyRequest) => Promise<Me>
  /** Yönetici anahtarı; yoksa yönetici uçları 404 */
  adminToken?: string
}

const LIST = 50

export function registerNews(app: FastifyInstance, ctx: NewsCtx): void {
  const { db, now, live, device } = ctx

  function admin(req: FastifyRequest) {
    const want = ctx.adminToken
    const h = req.headers.authorization ?? ''
    const got = h.startsWith('Bearer ') ? h.slice(7) : ''
    // Sabit süreli karşılaştırma (özetler aynı uzunlukta)
    if (!want || !timingSafeEqual(Buffer.from(sha256(got)), Buffer.from(sha256(want)))) {
      throw new HttpError(404, 'not found')
    }
  }

  app.get('/api/v1/notifications', async (req) => {
    await device(req)
    const rows = await db.query<{ id: string; title: string; body: string; created_at: Date }>(
      `select id, title, body, created_at from announcements order by id desc limit ${LIST}`,
    )
    return {
      items: rows.map((r) => ({ id: String(r.id), title: r.title, body: r.body, at: new Date(r.created_at).getTime() })),
    }
  })

  app.post<{ Body: { body: string; appVersion?: string } }>('/api/v1/feedback', {
    config: { rateLimit: { max: 5, timeWindow: '1 hour' } },
    schema: {
      body: {
        type: 'object',
        required: ['body'],
        properties: { body: { type: 'string', minLength: 1, maxLength: 4000 }, appVersion: { type: 'string', maxLength: 40 } },
      },
    },
  }, async (req) => {
    const me = await device(req)
    if (!req.body.body.trim()) throw new HttpError(400, 'empty feedback')
    await db.query(
      'insert into feedback (device_id, name_tag, body, app_version, created_at) values ($1, $2, $3, $4, to_timestamp($5 / 1000.0))',
      [me.id, `${me.hacker_name}#${me.tag}`, req.body.body.trim(), req.body.appVersion ?? null, now()],
    )
    return { ok: true }
  })

  // ------------------------------------------------------------ yönetici

  app.post<{ Body: { title: string; body: string } }>('/api/v1/admin/announcements', {
    schema: {
      body: {
        type: 'object',
        required: ['title', 'body'],
        properties: { title: { type: 'string', minLength: 1, maxLength: 120 }, body: { type: 'string', minLength: 1, maxLength: 4000 } },
      },
    },
  }, async (req) => {
    admin(req)
    const [row] = await db.query<{ id: string; created_at: Date }>(
      'insert into announcements (title, body, created_at) values ($1, $2, to_timestamp($3 / 1000.0)) returning id, created_at',
      [req.body.title, req.body.body, now()],
    )
    const at = new Date(row.created_at).getTime()
    live.broadcast({ type: 'announcement', id: String(row.id), title: req.body.title, body: req.body.body, at })
    return { id: String(row.id), at }
  })

  app.get('/api/v1/admin/feedback', async (req) => {
    admin(req)
    const rows = await db.query<{ id: string; name_tag: string; body: string; app_version: string | null; created_at: Date }>(
      'select id, name_tag, body, app_version, created_at from feedback order by id desc limit 200',
    )
    return { items: rows.map((r) => ({ ...r, id: String(r.id), created_at: new Date(r.created_at).getTime() })) }
  })
}
