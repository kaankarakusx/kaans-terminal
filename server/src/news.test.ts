import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { buildApp } from './app.ts'
import { type Db, migrate, pglite } from './db.ts'

const ADMIN = 'x'.repeat(40)
let db: Db
let app: ReturnType<typeof buildApp>

beforeEach(async () => {
  db = await pglite()
  await migrate(db)
  app = buildApp({ db, now: () => Date.UTC(2026, 10, 9), adminToken: ADMIN })
  await app.ready()
})
afterEach(async () => {
  await app.close()
  await db.close()
})

async function user() {
  const r = await app.inject({ method: 'POST', url: '/api/v1/devices', payload: { deviceHash: 'f'.repeat(64), hackerName: 'kaan' } })
  return r.json() as { token: string; name: string; tag: string }
}

describe('announcements', () => {
  it('only the admin can post; everyone connected gets it live; the list is newest first', async () => {
    const u = await user()
    const H = { authorization: `Bearer ${u.token}` }
    // kullanıcı token'ı yönetici değildir; yönetici uçları var olduğunu bile göstermez
    const denied = await app.inject({ method: 'POST', url: '/api/v1/admin/announcements', headers: H, payload: { title: 't', body: 'b' } })
    expect(denied.statusCode).toBe(404)

    const events: Array<Record<string, unknown>> = []
    const ws = await app.injectWS('/ws', { headers: H }, { onInit: (w) => w.on('message', (m: Buffer) => events.push(JSON.parse(String(m)))) })
    const A = { authorization: `Bearer ${ADMIN}` }
    for (const title of ['v0.2 is out', 'maintenance tonight']) {
      expect((await app.inject({ method: 'POST', url: '/api/v1/admin/announcements', headers: A, payload: { title, body: 'see @kaankarakusx' } })).statusCode).toBe(200)
    }
    await new Promise((r) => setTimeout(r, 50))
    expect(events.filter((e) => e.type === 'announcement').map((e) => e.title)).toEqual(['v0.2 is out', 'maintenance tonight'])
    const list = (await app.inject({ method: 'GET', url: '/api/v1/notifications', headers: H })).json()
    expect(list.items.map((i: { title: string }) => i.title)).toEqual(['maintenance tonight', 'v0.2 is out'])
    ws.terminate()
  })

  it('admin routes are closed when no admin token is configured', async () => {
    const closed = buildApp({ db, requireLive: false })
    const r = await closed.inject({ method: 'GET', url: '/api/v1/admin/feedback', headers: { authorization: 'Bearer ' } })
    expect(r.statusCode).toBe(404)
    await closed.close()
  })
})

describe('feedback', () => {
  it('stores feedback with name#tag (no email) for the admin to read', async () => {
    const u = await user()
    const H = { authorization: `Bearer ${u.token}` }
    expect((await app.inject({ method: 'POST', url: '/api/v1/feedback', headers: H, payload: { body: '  ' } })).statusCode).toBe(400)
    expect((await app.inject({ method: 'POST', url: '/api/v1/feedback', headers: H, payload: { body: 'orbit map is lovely', appVersion: '0.1.0' } })).statusCode).toBe(200)
    const r = (await app.inject({ method: 'GET', url: '/api/v1/admin/feedback', headers: { authorization: `Bearer ${ADMIN}` } })).json()
    expect(r.items[0]).toMatchObject({ name_tag: `kaan#${u.tag}`, body: 'orbit map is lovely', app_version: '0.1.0' })
  })
})
