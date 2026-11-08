import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { dayBaseXp } from '../../packages/shared/src/xp.ts'
import { buildApp } from './app.ts'
import { type Db, migrate, pglite } from './db.ts'

const HASH_A = 'a'.repeat(64)
const HASH_B = 'b'.repeat(64)

let db: Db
let clock: number
let app: ReturnType<typeof buildApp>

beforeEach(async () => {
  db = await pglite()
  await migrate(db)
  clock = Date.UTC(2026, 10, 9, 9, 0, 0) // pazartesi 09:00 UTC
  app = buildApp({ db, now: () => clock, requireLive: false })
})
afterEach(async () => {
  await app.close()
  await db.close()
})

async function register(hash: string, name: string) {
  const r = await app.inject({ method: 'POST', url: '/api/v1/devices', payload: { deviceHash: hash, hackerName: name } })
  return { status: r.statusCode, body: r.json() }
}
const auth = (token: string) => ({ authorization: `Bearer ${token}` })
async function beat(token: string, active = true, tz = 180) {
  const r = await app.inject({ method: 'POST', url: '/api/v1/heartbeat', headers: auth(token), payload: { active, tz } })
  expect(r.statusCode, r.body).toBe(200)
  return r.json()
}

describe('devices', () => {
  it('registers a device with a name#tag and a token; the hash cannot register twice', async () => {
    const a = await register(HASH_A, 'kaan')
    expect(a.status).toBe(201)
    expect(a.body.name).toBe('kaan')
    expect(a.body.tag).toMatch(/^\d{4}$/)
    expect(a.body.token.length).toBeGreaterThan(30)
    expect((await register(HASH_A, 'other')).status).toBe(409)
    const id = await app.inject({ method: 'POST', url: '/api/v1/devices/identify', payload: { deviceHash: HASH_A } })
    expect(id.json()).toEqual({ exists: true })
    // token saklanmaz, yalnızca özeti
    const rows = await db.query<{ token_hash: string }>('select token_hash from devices')
    expect(rows[0].token_hash).not.toBe(a.body.token)
  })

  it('rejects bad names and bad hashes', async () => {
    expect((await register(HASH_A, 'a')).status).toBe(400)
    expect((await register(HASH_A, 'bad name!')).status).toBe(400)
    expect((await register('nothex', 'kaan')).status).toBe(400)
  })

  it('same name gets a different tag; renaming keeps the tag when only the case changes', async () => {
    const a = await register(HASH_A, 'kaan')
    const b = await register(HASH_B, 'KAAN')
    expect(b.status).toBe(201)
    expect(b.body.tag).not.toBe(a.body.tag)
    const r = await app.inject({ method: 'PATCH', url: '/api/v1/me', headers: auth(a.body.token), payload: { hackerName: 'Kaan' } })
    expect(r.json()).toEqual({ name: 'Kaan', tag: a.body.tag })
  })

  it('requires a valid token', async () => {
    expect((await app.inject({ method: 'GET', url: '/api/v1/me/status' })).statusCode).toBe(401)
    expect((await app.inject({ method: 'GET', url: '/api/v1/me/status', headers: auth('nope') })).statusCode).toBe(401)
  })
})

describe('heartbeat', () => {
  it('credits active time with the server rule and computes XP on the server', async () => {
    const { body } = await register(HASH_A, 'kaan')
    const t = body.token
    expect((await beat(t)).credited).toBe(60) // ilk atış
    clock += 60_000
    expect((await beat(t)).credited).toBe(60)
    clock += 120_000
    expect((await beat(t)).credited).toBe(90) // en fazla 90
    clock += 600_000
    expect((await beat(t)).credited).toBe(60) // uzun boşluk
    clock += 60_000
    expect((await beat(t, false)).credited).toBe(0) // AFK
    const st = (await app.inject({ method: 'GET', url: '/api/v1/me/status', headers: auth(t) })).json()
    expect(st.todaySecs).toBe(270)
    expect(st.summary.totalXp).toBe(Math.floor(dayBaseXp(270)))
    expect(st.name).toBe('kaan')
    expect(st.last7).toHaveLength(7)
  })

  it('lets the time zone change at most once a day', async () => {
    const { body } = await register(HASH_A, 'kaan')
    expect((await beat(body.token, true, 180)).day).toBe('2026-11-09')
    clock += 60_000
    // Bugün zaten değişti (ilk atış 0 → 180): ikinci değişiklik yok sayılır
    expect((await beat(body.token, true, -600)).day).toBe('2026-11-09')
    const [p] = await db.query<{ tz_offset: number }>('select tz_offset from profiles')
    expect(p.tz_offset).toBe(180)
  })
})

describe('leaderboard', () => {
  it('ranks by XP this week, marks who is working now and honours privacy', async () => {
    const a = (await register(HASH_A, 'kaan')).body
    const b = (await register(HASH_B, 'ada')).body
    for (let i = 0; i < 5; i++) {
      await beat(a.token)
      await beat(b.token)
      clock += 60_000
    }
    await beat(b.token) // ada biraz önde
    await app.inject({ method: 'PATCH', url: '/api/v1/me/privacy', headers: auth(b.token), payload: { hideName: true, hideTime: true } })

    const r = (await app.inject({ method: 'GET', url: '/api/v1/leaderboard', headers: auth(a.token) })).json()
    expect(r.rows.map((x: { name: string }) => x.name)).toEqual(['Anonymous', 'kaan'])
    expect(r.rows[0].todaySecs).toBeNull() // süresini gizledi
    expect(r.rows[0].online).toBe(false)
    expect(r.rows[1]).toMatchObject({ you: true, online: true, tag: a.tag })
    expect(r.working).toBe(1)

    // 4 dakika sonra kimse "şu an çalışıyor" değil
    clock += 4 * 60_000
    const later = (await app.inject({ method: 'GET', url: '/api/v1/leaderboard', headers: auth(a.token) })).json()
    expect(later.working).toBe(0)
    // Sonraki hafta: haftalık XP sıfırlanır, tüm zamanlar kalır
    clock += 7 * 86_400_000
    const next = (await app.inject({ method: 'GET', url: '/api/v1/leaderboard?scope=week', headers: auth(a.token) })).json()
    expect(next.rows.map((x: { xp: number }) => x.xp)).toEqual([0])
    const all = (await app.inject({ method: 'GET', url: '/api/v1/leaderboard?scope=all', headers: auth(a.token) })).json()
    expect(all.rows).toHaveLength(2)
  })
})
