// Arkadaşlar, mesajlar, durum ve canlı bağlantı (OZELLIKLER.md §6.2, MIMARI.md §6.1).
// Arkadaşlık tek satır: (a < b), status 'pending' | 'accepted', isteği gönderen requested_by.
// Mesajlar yalnızca arkadaşlar arasında; sunucuda saklanır (uçtan uca şifreleme sonraki adım).
import type { FastifyInstance, FastifyRequest } from 'fastify'
import { ON_FIRE_MIN } from '../../packages/shared/src/xp.ts'
import type { Db } from './db.ts'
import type { Live } from './live.ts'
import { HttpError, localDay } from './util.ts'

export interface Me {
  id: string
  hacker_name: string
  tag: string
}

export interface SocialCtx {
  db: Db
  now: () => number
  live: Live
  device: (req: FastifyRequest) => Promise<Me>
}

const NAME_TAG = /^([A-Za-z0-9_.-]{3,20})#(\d{4})$/
/** Şifreli mesaj (hex): 2000 karakterlik metnin UTF-8'i (≤ 8000 bayt) + 16 bayt etiket → ≤ 16032 hex */
const MAX_CIPHER = 16_100
const HEX = (n?: number) => (n ? `^[0-9a-f]{${n}}$` : '^[0-9a-f]+$')
/** X25519 açık anahtarı: 32 bayt (64 hex); NaCl box nonce: 24 bayt (48 hex) */
const KEY_RE = HEX(64)
const NONCE_RE = HEX(48)
const PAGE = 50
export type Presence = 'online' | 'away' | 'invisible'

const pair = (x: string, y: string): [string, string] => (BigInt(x) < BigInt(y) ? [x, y] : [y, x])

export function registerSocial(app: FastifyInstance, ctx: SocialCtx): void {
  const { db, now, live, device } = ctx

  async function byNameTag(nameTag: string): Promise<Me> {
    const m = NAME_TAG.exec(nameTag.trim())
    if (!m) throw new HttpError(400, 'use name#tag, e.g. kaan#4242')
    const rows = await db.query<Me>('select id, hacker_name, tag from devices where name_key = $1 and tag = $2', [
      m[1].toLowerCase(),
      m[2],
    ])
    if (!rows[0]) throw new HttpError(404, `no one called ${nameTag}`)
    return { ...rows[0], id: String(rows[0].id) }
  }

  async function friendship(x: string, y: string) {
    const [a, b] = pair(x, y)
    const rows = await db.query<{ status: string; requested_by: string }>(
      'select status, requested_by from friendships where a = $1 and b = $2',
      [a, b],
    )
    return rows[0] ?? null
  }

  async function presenceOf(id: string): Promise<Presence> {
    const rows = await db.query<{ presence: Presence }>('select presence from profiles where device_id = $1', [id])
    return rows[0]?.presence ?? 'online'
  }

  /** Arkadaşlara görünen durum: görünmezse çevrimdışı */
  function visible(id: string, presence: Presence) {
    const online = live.isOnline(id) && presence !== 'invisible'
    return { online, away: online && (presence === 'away' || live.isAway(id)) }
  }

  async function friendIds(id: string): Promise<string[]> {
    const rows = await db.query<{ other: string }>(
      `select case when a = $1 then b else a end as other from friendships
        where (a = $1 or b = $1) and status = 'accepted'`,
      [id],
    )
    return rows.map((r) => String(r.other))
  }

  async function broadcastPresence(me: Me, notify: boolean) {
    const presence = await presenceOf(me.id)
    const v = visible(me.id, presence)
    for (const f of await friendIds(me.id)) {
      live.send(f, { type: 'presence', name: me.hacker_name, tag: me.tag, ...v, notify: notify && v.online })
    }
  }

  // ------------------------------------------------------------ arkadaş listesi

  app.get('/api/v1/friends', async (req) => {
    const me = await device(req)
    const t = now()
    const rows = await db.query<{
      other: string; status: string; requested_by: string; hacker_name: string; tag: string; level: number
      tz_offset: number; theme: string | null; hide_time: boolean; presence: Presence; public_key: string | null
    }>(
      `select d.id as other, f.status, f.requested_by, d.hacker_name, d.tag, p.level, p.tz_offset, p.theme,
              p.hide_time, p.presence, d.public_key
         from friendships f
         join devices d on d.id = case when f.a = $1 then f.b else f.a end
         join profiles p on p.device_id = d.id
        where f.a = $1 or f.b = $1
        order by lower(d.hacker_name)`,
      [me.id],
    )
    const accepted = rows.filter((r) => r.status === 'accepted')
    const ids = accepted.map((r) => String(r.other))
    const days = ids.length
      ? await db.query<{ device_id: string; day: string; active_seconds: number }>(
          'select device_id, day, active_seconds from activity_days where device_id = any($1) and day >= $2',
          [ids, localDay(t - 8 * 86_400_000, -720)],
        )
      : []
    const unread = await db.query<{ from_id: string; n: string }>(
      'select from_id, count(*) as n from messages where to_id = $1 and read_at is null group by from_id',
      [me.id],
    )
    const friends = accepted.map((r) => {
      const id = String(r.other)
      const mine = days.filter((d) => String(d.device_id) === id)
      const l7: number[] = []
      for (let i = 6; i >= 0; i--) {
        const day = localDay(t - i * 86_400_000, r.tz_offset)
        l7.push(Number(mine.find((d) => d.day === day)?.active_seconds ?? 0))
      }
      const v = visible(id, r.presence)
      return {
        name: r.hacker_name,
        tag: r.tag,
        level: r.level,
        theme: r.theme,
        ...v,
        todaySecs: r.hide_time ? null : l7[6],
        last7: r.hide_time ? null : l7,
        onFire: !r.hide_time && l7[6] >= ON_FIRE_MIN * 60,
        unread: Number(unread.find((u) => String(u.from_id) === id)?.n ?? 0),
        /** Uçtan uca şifreleme için (yoksa arkadaş henüz anahtar yüklememiş) */
        publicKey: r.public_key,
      }
    })
    const pending = rows.filter((r) => r.status === 'pending')
    return {
      friends,
      incoming: pending.filter((r) => String(r.requested_by) !== String(me.id)).map((r) => ({ name: r.hacker_name, tag: r.tag })),
      outgoing: pending.filter((r) => String(r.requested_by) === String(me.id)).map((r) => ({ name: r.hacker_name, tag: r.tag })),
    }
  })

  // ------------------------------------------------------------ istekler

  const nameTagBody = {
    schema: { body: { type: 'object', required: ['nameTag'], properties: { nameTag: { type: 'string', maxLength: 30 } } } },
  }

  /** İstek gönder. Karşı taraf zaten istek gönderdiyse doğrudan arkadaş olunur. */
  app.post<{ Body: { nameTag: string } }>('/api/v1/friends', { ...nameTagBody, config: { rateLimit: { max: 30, timeWindow: '1 hour' } } }, async (req) => {
    const me = await device(req)
    const other = await byNameTag(req.body.nameTag)
    if (String(other.id) === String(me.id)) throw new HttpError(400, "that's you")
    const f = await friendship(me.id, other.id)
    const [a, b] = pair(me.id, other.id)
    if (f?.status === 'accepted') return { status: 'friends' }
    if (f && String(f.requested_by) === String(me.id)) return { status: 'sent' }
    if (f) {
      await db.query(`update friendships set status = 'accepted' where a = $1 and b = $2`, [a, b])
      live.send(other.id, { type: 'friend', name: me.hacker_name, tag: me.tag, status: 'accepted' })
      return { status: 'friends' }
    }
    await db.query(`insert into friendships (a, b, status, requested_by) values ($1, $2, 'pending', $3)`, [a, b, me.id])
    live.send(other.id, { type: 'request', from: { name: me.hacker_name, tag: me.tag } })
    return { status: 'sent' }
  })

  app.post<{ Body: { nameTag: string } }>('/api/v1/friends/accept', nameTagBody, async (req) => {
    const me = await device(req)
    const other = await byNameTag(req.body.nameTag)
    const f = await friendship(me.id, other.id)
    if (!f || f.status !== 'pending' || String(f.requested_by) === String(me.id)) throw new HttpError(404, 'no request from them')
    const [a, b] = pair(me.id, other.id)
    await db.query(`update friendships set status = 'accepted' where a = $1 and b = $2`, [a, b])
    live.send(other.id, { type: 'friend', name: me.hacker_name, tag: me.tag, status: 'accepted' })
    return { status: 'friends' }
  })

  /** Reddet, isteği geri çek ya da arkadaşlıktan çıkar */
  app.post<{ Body: { nameTag: string } }>('/api/v1/friends/remove', nameTagBody, async (req) => {
    const me = await device(req)
    const other = await byNameTag(req.body.nameTag)
    const [a, b] = pair(me.id, other.id)
    const removed = await db.query('delete from friendships where a = $1 and b = $2 returning status', [a, b])
    if (removed.length) live.send(other.id, { type: 'friend', name: me.hacker_name, tag: me.tag, status: 'removed' })
    return { status: 'none' }
  })

  // ------------------------------------------------------------ durum

  app.patch<{ Body: { presence: Presence } }>('/api/v1/me/presence', {
    schema: { body: { type: 'object', required: ['presence'], properties: { presence: { type: 'string', enum: ['online', 'away', 'invisible'] } } } },
  }, async (req) => {
    const me = await device(req)
    await db.query('update profiles set presence = $1 where device_id = $2', [req.body.presence, me.id])
    await broadcastPresence(me, false)
    return { presence: req.body.presence }
  })

  // ------------------------------------------------------------ anahtar

  /** Cihazın açık anahtarı (gizli anahtar cihazdan çıkmaz) */
  app.put<{ Body: { publicKey: string } }>('/api/v1/me/key', {
    config: { rateLimit: { max: 10, timeWindow: '1 hour' } },
    schema: { body: { type: 'object', required: ['publicKey'], properties: { publicKey: { type: 'string', pattern: KEY_RE } } } },
  }, async (req) => {
    const me = await device(req)
    await db.query('update devices set public_key = $1 where id = $2', [req.body.publicKey, me.id])
    return { ok: true }
  })

  async function keyOf(id: string): Promise<string | null> {
    const rows = await db.query<{ public_key: string | null }>('select public_key from devices where id = $1', [id])
    return rows[0]?.public_key ?? null
  }

  // ------------------------------------------------------------ mesajlar (uçtan uca şifreli)

  async function mustBeFriends(me: Me, other: Me) {
    const f = await friendship(me.id, other.id)
    if (f?.status !== 'accepted') throw new HttpError(403, 'you can only message friends')
  }

  app.get<{ Querystring: { with: string; before?: string } }>('/api/v1/messages', {
    schema: {
      querystring: {
        type: 'object',
        required: ['with'],
        properties: { with: { type: 'string', maxLength: 30 }, before: { type: 'string', pattern: '^\\d+$' } },
      },
    },
  }, async (req) => {
    const me = await device(req)
    const other = await byNameTag(req.query.with)
    await mustBeFriends(me, other)
    const rows = await db.query<{
      id: string; from_id: string; body: string; nonce: string; from_key: string; to_key: string; created_at: Date; read_at: Date | null
    }>(
      `select id, from_id, body, nonce, from_key, to_key, created_at, read_at from messages
        where ((from_id = $1 and to_id = $2) or (from_id = $2 and to_id = $1))
          and ($3::bigint is null or id < $3::bigint)
        order by id desc limit ${PAGE}`,
      [me.id, other.id, req.query.before ?? null],
    )
    await db.query('update messages set read_at = now() where to_id = $1 and from_id = $2 and read_at is null', [me.id, other.id])
    return {
      messages: rows.reverse().map((r) => ({
        id: String(r.id),
        mine: String(r.from_id) === String(me.id),
        ciphertext: r.body,
        nonce: r.nonce,
        fromKey: r.from_key,
        toKey: r.to_key,
        at: new Date(r.created_at).getTime(),
        read: !!r.read_at,
      })),
      more: rows.length === PAGE,
    }
  })

  /**
   * Şifreli mesaj: istemci alıcının açık anahtarıyla şifreler ve hangi anahtarı kullandığını (`toKey`) söyler.
   * Alıcının anahtarı o arada değiştiyse 409: istemci yeni anahtarı alıp yeniden şifreler (yanlış anahtara gitmez).
   */
  app.post<{ Body: { to: string; ciphertext: string; nonce: string; toKey: string } }>('/api/v1/messages', {
    config: { rateLimit: { max: 30, timeWindow: '1 minute' } },
    schema: {
      body: {
        type: 'object',
        required: ['to', 'ciphertext', 'nonce', 'toKey'],
        properties: {
          to: { type: 'string', maxLength: 30 },
          ciphertext: { type: 'string', minLength: 34, maxLength: MAX_CIPHER, pattern: HEX() },
          nonce: { type: 'string', pattern: NONCE_RE },
          toKey: { type: 'string', pattern: KEY_RE },
        },
      },
    },
  }, async (req) => {
    const me = await device(req)
    const other = await byNameTag(req.body.to)
    await mustBeFriends(me, other)
    const fromKey = await keyOf(me.id)
    if (!fromKey) throw new HttpError(409, 'upload your public key first')
    const toKey = await keyOf(other.id)
    if (!toKey) throw new HttpError(409, `${other.hacker_name} hasn't set up encryption yet`)
    if (toKey !== req.body.toKey) throw new HttpError(409, 'their key changed, encrypt again')
    const [row] = await db.query<{ id: string; created_at: Date }>(
      `insert into messages (from_id, to_id, body, nonce, from_key, to_key, created_at)
       values ($1, $2, $3, $4, $5, $6, to_timestamp($7 / 1000.0)) returning id, created_at`,
      [me.id, other.id, req.body.ciphertext, req.body.nonce, fromKey, toKey, now()],
    )
    const at = new Date(row.created_at).getTime()
    live.send(other.id, {
      type: 'message',
      id: String(row.id),
      from: { name: me.hacker_name, tag: me.tag },
      ciphertext: req.body.ciphertext,
      nonce: req.body.nonce,
      fromKey,
      toKey,
      at,
    })
    return { id: String(row.id), at }
  })

  // ------------------------------------------------------------ canlı bağlantı

  app.get('/ws', { websocket: true }, async (socket, req) => {
    let me: Me
    try {
      me = await device(req)
    } catch {
      socket.close(4401, 'unauthorized')
      return
    }
    const { longAway } = live.add(me.id, socket, now())
    socket.send(JSON.stringify({ type: 'hello', name: me.hacker_name, tag: me.tag }))
    await broadcastPresence(me, longAway)
    socket.on('message', async (raw: Buffer) => {
      try {
        const msg = JSON.parse(String(raw)) as { type?: string; away?: unknown }
        if (msg.type === 'away' && typeof msg.away === 'boolean' && live.setAway(me.id, msg.away)) {
          await broadcastPresence(me, false)
        }
      } catch {
        /* geçersiz çerçeve: yok say */
      }
    })
    socket.on('close', async () => {
      if (live.remove(me.id, socket, now())) await broadcastPresence(me, false)
    })
  })
}
