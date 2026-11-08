// HTTP API (MIMARI.md §6.1). Hesap sunucuda yapılır; istemci yalnızca "aktif miydim" der. XP kuralları
// packages/shared/src/xp.ts (uygulamayla aynı kod).
import rateLimit from '@fastify/rate-limit'
import websocket from '@fastify/websocket'
import Fastify, { type FastifyInstance, type FastifyRequest } from 'fastify'
import {
  ON_FIRE_MIN, heartbeatCredit, restedLeftMin, summarize, totalXp, weekOf, type DayActivity,
} from '../../packages/shared/src/xp.ts'
import type { Db } from './db.ts'
import { registerSocial } from './friends.ts'
import { Live } from './live.ts'
import { registerNews } from './news.ts'
import { HttpError, NAME_RE, localDay, newToken, randomTag, sha256 } from './util.ts'

export interface AppOptions {
  db: Db
  /** Saat (testlerde sahte) */
  now?: () => number
  /** Cloudflare arkasında: istemci IP'si CF-Connecting-IP'den */
  trustProxy?: boolean
  logger?: boolean
  /** Hile önlemi: XP yalnızca canlı bağlantı (WS) açıkken sayılır. Testlerde kapatılabilir. */
  requireLive?: boolean
  /** Yönetici anahtarı (duyuru yazma, geri bildirimi okuma); yoksa yönetici uçları kapalı */
  adminToken?: string
}

interface Device {
  id: string
  hacker_name: string
  tag: string
}

/** Son 3 dakikada aktif atış: "şu an çalışıyor" */
const LIVE_MS = 3 * 60_000
const LEADERBOARD_SIZE = 100

export function buildApp(opts: AppOptions): FastifyInstance {
  const { db } = opts
  const now = opts.now ?? Date.now
  const app = Fastify({
    logger: opts.logger ?? false,
    bodyLimit: 16 * 1024,
    trustProxy: opts.trustProxy ?? false,
  })

  const live = new Live()
  const requireLive = opts.requireLive ?? true
  app.addHook('onClose', async () => live.closeAll())
  void app.register(websocket, { options: { maxPayload: 4096 } })
  void app.register(rateLimit, {
    max: 120,
    timeWindow: '1 minute',
    keyGenerator: (req) => (req.headers['cf-connecting-ip'] as string | undefined) ?? req.ip,
  })

  app.setErrorHandler((err: Error & { statusCode?: number; validation?: unknown }, _req, reply) => {
    if (err instanceof HttpError) return reply.status(err.status).send({ error: err.message })
    if (err.validation) return reply.status(400).send({ error: err.message })
    if (err.statusCode && err.statusCode < 500) return reply.status(err.statusCode).send({ error: err.message })
    app.log.error(err)
    return reply.status(500).send({ error: 'internal error' })
  })

  async function device(req: FastifyRequest): Promise<Device> {
    const h = req.headers.authorization
    const token = h?.startsWith('Bearer ') ? h.slice(7).trim() : ''
    if (!token) throw new HttpError(401, 'missing token')
    const rows = await db.query<Device>('select id, hacker_name, tag from devices where token_hash = $1', [sha256(token)])
    if (!rows[0]) throw new HttpError(401, 'unknown token')
    // Kimlik her yerde metin (sürücüler bigint'i farklı döndürür; canlı bağlantı kaydı bununla anahtarlanır)
    return { ...rows[0], id: String(rows[0].id) }
  }

  async function freeTag(q: Db['query'], nameKey: string): Promise<string> {
    for (let i = 0; i < 60; i++) {
      const tag = randomTag()
      const taken = await q('select 1 from devices where name_key = $1 and tag = $2', [nameKey, tag])
      if (!taken.length) return tag
    }
    throw new HttpError(409, 'this name is too popular, pick another')
  }

  function checkName(name: unknown): string {
    if (typeof name !== 'string' || !NAME_RE.test(name)) throw new HttpError(400, 'name: 3–20 letters, digits, _ . -')
    return name
  }

  async function daysOf(deviceId: string): Promise<DayActivity[]> {
    const rows = await db.query<{ day: string; active_seconds: number }>(
      'select day, active_seconds from activity_days where device_id = $1 order by day',
      [deviceId],
    )
    return rows.map((r) => ({ day: r.day, activeSecs: Number(r.active_seconds) }))
  }

  /** Bu hafta (oyuncunun kendi pazartesisinden) kazanılan XP */
  function weekXp(days: DayActivity[], today: string): number {
    const start = weekOf(today)
    return totalXp(days, today).total - totalXp(days.filter((d) => d.day < start), today).total
  }

  function last7(days: DayActivity[], nowMs: number, tz: number): number[] {
    const out: number[] = []
    for (let i = 6; i >= 0; i--) {
      const d = localDay(nowMs - i * 86_400_000, tz)
      out.push(days.find((x) => x.day === d)?.activeSecs ?? 0)
    }
    return out
  }

  // ------------------------------------------------------------ cihaz kaydı

  app.post<{ Body: { deviceHash: string } }>('/api/v1/devices/identify', {
    schema: { body: { type: 'object', required: ['deviceHash'], properties: { deviceHash: { type: 'string', pattern: '^[0-9a-f]{64}$' } } } },
  }, async (req) => {
    const rows = await db.query('select 1 from devices where device_hash = $1', [req.body.deviceHash])
    return { exists: rows.length > 0 }
  })

  app.post<{ Body: { deviceHash: string; hackerName: string; os?: string; appVersion?: string } }>('/api/v1/devices', {
    config: { rateLimit: { max: 10, timeWindow: '1 hour' } },
    schema: {
      body: {
        type: 'object',
        required: ['deviceHash', 'hackerName'],
        properties: {
          deviceHash: { type: 'string', pattern: '^[0-9a-f]{64}$' },
          hackerName: { type: 'string', maxLength: 20 },
          os: { type: 'string', maxLength: 40 },
          appVersion: { type: 'string', maxLength: 40 },
        },
      },
    },
  }, async (req, reply) => {
    const name = checkName(req.body.hackerName)
    const token = newToken()
    const res = await db.tx(async (q) => {
      if ((await q('select 1 from devices where device_hash = $1', [req.body.deviceHash])).length) {
        throw new HttpError(409, 'device already registered')
      }
      const tag = await freeTag(q, name.toLowerCase())
      const [row] = await q<{ id: string }>(
        `insert into devices (device_hash, token_hash, hacker_name, name_key, tag, os, app_version, last_seen)
         values ($1, $2, $3, $4, $5, $6, $7, now()) returning id`,
        [req.body.deviceHash, sha256(token), name, name.toLowerCase(), tag, req.body.os ?? null, req.body.appVersion ?? null],
      )
      await q('insert into profiles (device_id) values ($1)', [row.id])
      return { token, name, tag }
    })
    return reply.status(201).send(res)
  })

  app.patch<{ Body: { hackerName: string } }>('/api/v1/me', {
    config: { rateLimit: { max: 10, timeWindow: '1 hour' } },
    schema: { body: { type: 'object', required: ['hackerName'], properties: { hackerName: { type: 'string', maxLength: 20 } } } },
  }, async (req) => {
    const me = await device(req)
    const name = checkName(req.body.hackerName)
    return db.tx(async (q) => {
      const tag = name.toLowerCase() === me.hacker_name.toLowerCase() ? me.tag : await freeTag(q, name.toLowerCase())
      await q('update devices set hacker_name = $1, name_key = $2, tag = $3 where id = $4', [name, name.toLowerCase(), tag, me.id])
      return { name, tag }
    })
  })

  app.patch<{ Body: { hideTime?: boolean; hideName?: boolean } }>('/api/v1/me/privacy', {
    schema: { body: { type: 'object', properties: { hideTime: { type: 'boolean' }, hideName: { type: 'boolean' } } } },
  }, async (req) => {
    const me = await device(req)
    await db.query(
      'update profiles set hide_time = coalesce($1, hide_time), hide_name = coalesce($2, hide_name) where device_id = $3',
      [req.body.hideTime ?? null, req.body.hideName ?? null, me.id],
    )
    return { ok: true }
  })

  // ------------------------------------------------------------ heartbeat

  app.post<{ Body: { active: boolean; tz: number; theme?: string; appVersion?: string } }>('/api/v1/heartbeat', {
    config: { rateLimit: { max: 20, timeWindow: '1 minute' } },
    schema: {
      body: {
        type: 'object',
        required: ['active', 'tz'],
        properties: {
          active: { type: 'boolean' },
          tz: { type: 'integer', minimum: -720, maximum: 840 },
          theme: { type: 'string', maxLength: 40 },
          appVersion: { type: 'string', maxLength: 40 },
        },
      },
    },
  }, async (req) => {
    const me = await device(req)
    const t = now()
    const summary = await db.tx(async (q) => {
      const [p] = await q<{ tz_offset: number; tz_changed_day: string | null; last_hb: Date | null }>(
        'select tz_offset, tz_changed_day, last_hb from profiles where device_id = $1 for update',
        [me.id],
      )
      // Saat dilimi günde en fazla bir kez değişebilir (hile önlemi)
      let tz = p.tz_offset
      let tzDay = p.tz_changed_day
      if (req.body.tz !== tz && tzDay !== localDay(t, tz)) {
        tz = req.body.tz
        tzDay = localDay(t, tz)
      }
      const gap = p.last_hb ? Math.max(0, Math.floor((t - new Date(p.last_hb).getTime()) / 1000)) : 60
      // Süre yalnızca canlı bağlantı açıkken ve AFK değilken sayılır (OZELLIKLER.md §6.3 hile önlemleri)
      const credit = req.body.active && (!requireLive || live.isOnline(me.id)) ? heartbeatCredit(gap) : 0
      const day = localDay(t, tz)
      if (credit > 0) {
        await q(
          `insert into activity_days (device_id, day, active_seconds) values ($1, $2, $3)
           on conflict (device_id, day) do update set active_seconds = activity_days.active_seconds + excluded.active_seconds`,
          [me.id, day, credit],
        )
      }
      const days = (await q<{ day: string; active_seconds: number }>(
        'select day, active_seconds from activity_days where device_id = $1 order by day', [me.id],
      )).map((r) => ({ day: r.day, activeSecs: Number(r.active_seconds) }))
      const s = summarize(days, day)
      await q(
        `update profiles set tz_offset = $1, tz_changed_day = $2, last_hb = to_timestamp($3 / 1000.0),
           last_active = case when $4 then to_timestamp($3 / 1000.0) else last_active end,
           theme = coalesce($5, theme), xp_total = $6, level = $7, week_start = $8, week_xp = $9
         where device_id = $10`,
        [tz, tzDay, t, req.body.active, req.body.theme ?? null, s.totalXp, s.level, weekOf(day), weekXp(days, day), me.id],
      )
      await q('update devices set last_seen = to_timestamp($1 / 1000.0), app_version = coalesce($2, app_version) where id = $3', [
        t, req.body.appVersion ?? null, me.id,
      ])
      return { ...s, credited: credit, day }
    })
    return summary
  })

  // ------------------------------------------------------------ profil

  app.get('/api/v1/me/status', async (req) => {
    const me = await device(req)
    const t = now()
    const [p] = await db.query<{ tz_offset: number; theme: string | null; hide_time: boolean; hide_name: boolean }>(
      'select tz_offset, theme, hide_time, hide_name from profiles where device_id = $1', [me.id],
    )
    const days = await daysOf(me.id)
    const today = localDay(t, p.tz_offset)
    const todaySecs = days.find((d) => d.day === today)?.activeSecs ?? 0
    const week = weekOf(today)
    return {
      name: me.hacker_name,
      tag: me.tag,
      theme: p.theme,
      hideTime: p.hide_time,
      hideName: p.hide_name,
      summary: summarize(days, today),
      weekXp: weekXp(days, today),
      todaySecs,
      weekSecs: days.filter((d) => weekOf(d.day) === week).reduce((a, d) => a + d.activeSecs, 0),
      last7: last7(days, t, p.tz_offset),
      restedLeftMin: restedLeftMin(todaySecs),
      onFire: todaySecs >= ON_FIRE_MIN * 60,
    }
  })

  // ------------------------------------------------------------ sıralama

  app.get<{ Querystring: { scope?: 'week' | 'all' } }>('/api/v1/leaderboard', {
    schema: { querystring: { type: 'object', properties: { scope: { type: 'string', enum: ['week', 'all'] } } } },
  }, async (req) => {
    const me = await device(req)
    const t = now()
    const scope = req.query.scope ?? 'week'
    const rows = await db.query<{
      id: string; hacker_name: string; tag: string; tz_offset: number; theme: string | null; hide_time: boolean
      hide_name: boolean; xp_total: number; level: number; week_start: string | null; week_xp: number; last_active: Date | null
    }>(
      `select d.id, d.hacker_name, d.tag, p.tz_offset, p.theme, p.hide_time, p.hide_name, p.xp_total, p.level,
              p.week_start, p.week_xp, p.last_active
         from devices d join profiles p on p.device_id = d.id
        where p.xp_total > 0 or d.id = $1`,
      [me.id],
    )
    const scored = rows
      .map((r) => {
        // Hafta, oyuncunun kendi pazartesisinden; o hafta hiç atış yoksa 0
        const currentWeek = weekOf(localDay(t, r.tz_offset))
        const xp = scope === 'week' ? (r.week_start === currentWeek ? r.week_xp : 0) : r.xp_total
        return { r, xp }
      })
      .filter((x) => x.xp > 0 || String(x.r.id) === me.id)
      .sort((a, b) => b.xp - a.xp)
      .slice(0, LEADERBOARD_SIZE)
    const ids = scored.map((x) => x.r.id)
    const since = localDay(t - 8 * 86_400_000, -720)
    const dayRows = ids.length
      ? await db.query<{ device_id: string; day: string; active_seconds: number }>(
          'select device_id, day, active_seconds from activity_days where device_id = any($1) and day >= $2',
          [ids, since],
        )
      : []
    // Sıralamadan tek tıkla istek: her satırın benimle arkadaşlık durumu
    const rel = await db.query<{ other: string; status: string; requested_by: string }>(
      `select case when a = $1 then b else a end as other, status, requested_by from friendships where a = $1 or b = $1`,
      [me.id],
    )
    const friendOf = (id: string) => {
      const f = rel.find((x) => String(x.other) === String(id))
      if (!f) return 'none'
      if (f.status === 'accepted') return 'friends'
      return String(f.requested_by) === String(me.id) ? 'sent' : 'received'
    }
    let working = 0
    const list = scored.map(({ r, xp }) => {
      const you = String(r.id) === me.id
      const days = dayRows.filter((d) => d.device_id === r.id).map((d) => ({ day: d.day, activeSecs: Number(d.active_seconds) }))
      const live = !!r.last_active && t - new Date(r.last_active).getTime() <= LIVE_MS
      const showTime = you || !r.hide_time
      if (live && showTime) working++
      const l7 = last7(days, t, r.tz_offset)
      return {
        name: you || !r.hide_name ? r.hacker_name : 'Anonymous',
        tag: you || !r.hide_name ? r.tag : '',
        you,
        friend: you ? 'self' : friendOf(r.id),
        level: r.level,
        xp,
        theme: r.theme,
        online: showTime && live,
        onFire: showTime && l7[6] >= ON_FIRE_MIN * 60,
        todaySecs: showTime ? l7[6] : null,
        last7: showTime ? l7 : null,
      }
    })
    return { scope, working, rows: list }
  })

  void app.register(async (scope) => registerSocial(scope, { db, now, live, device }))
  void app.register(async (scope) => registerNews(scope, { db, now, live, device, adminToken: opts.adminToken }))

  app.get('/api/v1/health', async () => ({ ok: true }))

  return app
}
