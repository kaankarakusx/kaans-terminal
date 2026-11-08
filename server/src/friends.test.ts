import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { buildApp } from './app.ts'
import { type Db, migrate, pglite } from './db.ts'

let db: Db
let clock: number
let app: ReturnType<typeof buildApp>

beforeEach(async () => {
  db = await pglite()
  await migrate(db)
  clock = Date.UTC(2026, 10, 9, 9, 0, 0)
  app = buildApp({ db, now: () => clock })
  await app.ready()
})
afterEach(async () => {
  await app.close()
  await db.close()
})

interface User { token: string; name: string; tag: string; id: string }
let n = 0
async function user(name: string): Promise<User> {
  const hash = String(++n).padStart(64, '0')
  const r = await app.inject({ method: 'POST', url: '/api/v1/devices', payload: { deviceHash: hash, hackerName: name } })
  expect(r.statusCode, r.body).toBe(201)
  return { ...r.json(), id: String(n) }
}
const H = (u: User) => ({ authorization: `Bearer ${u.token}` })
const nt = (u: User) => `${u.name}#${u.tag}`
async function call(u: User, method: 'GET' | 'POST' | 'PATCH', url: string, payload?: object) {
  const r = await app.inject({ method, url, headers: H(u), payload })
  return { status: r.statusCode, body: r.json() }
}

/** WS bağlan, gelen olayları topla */
async function connect(u: User) {
  const events: Array<Record<string, unknown>> = []
  // Dinleyici bağlantı açılmadan bağlanır (sunucu "hello"yu hemen gönderir)
  const ws = await app.injectWS('/ws', { headers: H(u) }, {
    onInit: (w) => w.on('message', (m: Buffer) => events.push(JSON.parse(String(m)))),
  })
  const next = async (type: string) => {
    for (let i = 0; i < 100; i++) {
      const e = events.find((x) => x.type === type)
      if (e) {
        events.splice(events.indexOf(e), 1)
        return e
      }
      await new Promise((r) => setTimeout(r, 10))
    }
    throw new Error(`no ${type} event; got ${JSON.stringify(events)}`)
  }
  await next('hello')
  return { ws, events, next }
}

describe('friends', () => {
  it('request → accept → listed; requests both ways auto-accept; remove', async () => {
    const kaan = await user('kaan')
    const ada = await user('ada')
    const bob = await user('bob')
    expect((await call(kaan, 'POST', '/api/v1/friends', { nameTag: nt(ada) })).body).toEqual({ status: 'sent' })
    expect((await call(kaan, 'POST', '/api/v1/friends', { nameTag: nt(kaan) })).status).toBe(400)
    expect((await call(kaan, 'POST', '/api/v1/friends', { nameTag: 'ghost#0000' })).status).toBe(404)

    const adaView = (await call(ada, 'GET', '/api/v1/friends')).body
    expect(adaView.incoming).toEqual([{ name: 'kaan', tag: kaan.tag }])
    expect((await call(kaan, 'GET', '/api/v1/friends')).body.outgoing).toEqual([{ name: 'ada', tag: ada.tag }])

    expect((await call(ada, 'POST', '/api/v1/friends/accept', { nameTag: nt(kaan) })).body).toEqual({ status: 'friends' })
    const list = (await call(kaan, 'GET', '/api/v1/friends')).body
    expect(list.friends.map((f: { name: string }) => f.name)).toEqual(['ada'])
    expect(list.friends[0]).toMatchObject({ online: false, unread: 0, last7: [0, 0, 0, 0, 0, 0, 0] })

    // Karşılıklı istek: ikinci istek kabul sayılır
    await call(kaan, 'POST', '/api/v1/friends', { nameTag: nt(bob) })
    expect((await call(bob, 'POST', '/api/v1/friends', { nameTag: nt(kaan) })).body).toEqual({ status: 'friends' })

    await call(kaan, 'POST', '/api/v1/friends/remove', { nameTag: nt(bob) })
    expect((await call(kaan, 'GET', '/api/v1/friends')).body.friends).toHaveLength(1)

    // Sıralamada arkadaşlık durumu
    const board = (await call(kaan, 'GET', '/api/v1/leaderboard?scope=all')).body
    expect(board.rows.find((r: { you: boolean }) => r.you).friend).toBe('self')
  })
})

const KEY = (c: string) => c.repeat(64)
const NONCE = '0'.repeat(48)
const CT = (n: number) => 'ab'.repeat(17 + n)
async function setKey(u: User, k: string) {
  expect((await app.inject({ method: 'PUT', url: '/api/v1/me/key', headers: H(u), payload: { publicKey: k } })).statusCode).toBe(200)
}

describe('messages (end-to-end encrypted)', () => {
  it('only friends; needs both keys; refuses a stale recipient key; history carries ciphertext and keys', async () => {
    const kaan = await user('kaan')
    const ada = await user('ada')
    const msg = (n: number, toKey = KEY('a')) => ({ to: nt(ada), ciphertext: CT(n), nonce: NONCE, toKey })
    expect((await call(kaan, 'POST', '/api/v1/messages', msg(1))).status).toBe(403)
    await call(kaan, 'POST', '/api/v1/friends', { nameTag: nt(ada) })
    await call(ada, 'POST', '/api/v1/friends/accept', { nameTag: nt(kaan) })

    // anahtarlar yokken
    expect((await call(kaan, 'POST', '/api/v1/messages', msg(1))).status).toBe(409)
    await setKey(kaan, KEY('b'))
    expect((await call(kaan, 'POST', '/api/v1/messages', msg(1))).body.error).toMatch(/hasn't set up encryption/)
    await setKey(ada, KEY('a'))
    // düz metin ya da bozuk alanlar kabul edilmez
    expect((await call(kaan, 'POST', '/api/v1/messages', { to: nt(ada), body: 'selam' })).status).toBe(400)
    expect((await call(kaan, 'POST', '/api/v1/messages', { ...msg(1), nonce: 'zz' })).status).toBe(400)
    // eski anahtarla şifrelenmiş: reddedilir
    expect((await call(kaan, 'POST', '/api/v1/messages', msg(1, KEY('c')))).body.error).toMatch(/key changed/)

    for (const n of [1, 2]) expect((await call(kaan, 'POST', '/api/v1/messages', msg(n))).status).toBe(200)
    expect((await call(ada, 'GET', '/api/v1/friends')).body.friends[0]).toMatchObject({ unread: 2, publicKey: KEY('b') })
    const hist = (await call(ada, 'GET', `/api/v1/messages?with=${encodeURIComponent(nt(kaan))}`)).body
    expect(hist.messages.map((m: Record<string, unknown>) => [m.ciphertext, m.mine, m.fromKey, m.toKey])).toEqual([
      [CT(1), false, KEY('b'), KEY('a')],
      [CT(2), false, KEY('b'), KEY('a')],
    ])
    expect(JSON.stringify(hist)).not.toContain('selam')
    expect((await call(ada, 'GET', '/api/v1/friends')).body.friends[0].unread).toBe(0)
  })
})

describe('live', () => {
  it('pushes messages, requests and presence; "is now online" only after a long absence; invisible hides you', async () => {
    const kaan = await user('kaan')
    const ada = await user('ada')
    const k = await connect(kaan)

    await call(ada, 'POST', '/api/v1/friends', { nameTag: nt(kaan) })
    expect(await k.next('request')).toMatchObject({ from: { name: 'ada', tag: ada.tag } })
    await call(kaan, 'POST', '/api/v1/friends/accept', { nameTag: nt(ada) })

    // ada ilk kez bağlanıyor → kaan "is now online" alır
    const a = await connect(ada)
    expect(await k.next('presence')).toMatchObject({ name: 'ada', online: true, notify: true })
    expect((await call(kaan, 'GET', '/api/v1/friends')).body.friends[0].online).toBe(true)

    // şifreli mesaj anında gelir (sunucu yalnızca şifreli metni iletir)
    await setKey(kaan, KEY('b'))
    await setKey(ada, KEY('a'))
    await call(ada, 'POST', '/api/v1/messages', { to: nt(kaan), ciphertext: CT(3), nonce: NONCE, toKey: KEY('b') })
    expect(await k.next('message')).toMatchObject({ ciphertext: CT(3), nonce: NONCE, fromKey: KEY('a'), from: { name: 'ada' } })

    // AFK
    a.ws.send(JSON.stringify({ type: 'away', away: true }))
    expect(await k.next('presence')).toMatchObject({ online: true, away: true, notify: false })

    // kısa kopma: tekrar bağlanınca bildirim yok
    a.ws.terminate()
    expect(await k.next('presence')).toMatchObject({ online: false })
    clock += 60_000
    const a2 = await connect(ada)
    expect(await k.next('presence')).toMatchObject({ online: true, notify: false })

    // görünmez: arkadaşlara çevrimdışı görünür
    await call(ada, 'PATCH', '/api/v1/me/presence', { presence: 'invisible' })
    expect(await k.next('presence')).toMatchObject({ online: false })
    expect((await call(kaan, 'GET', '/api/v1/friends')).body.friends[0].online).toBe(false)
    a2.ws.terminate()
    k.ws.terminate()
  })

  it('rejects connections without a valid token', async () => {
    const ws = await app.injectWS('/ws', { headers: { authorization: 'Bearer nope' } })
    const code = await new Promise((r) => ws.on('close', (c: number) => r(c)))
    expect(code).toBe(4401)
  })

  it('credits XP only while the live connection is open', async () => {
    const kaan = await user('kaan')
    const beat = () => call(kaan, 'POST', '/api/v1/heartbeat', { active: true, tz: 180 })
    expect((await beat()).body.credited).toBe(0)
    const k = await connect(kaan)
    clock += 60_000
    expect((await beat()).body.credited).toBe(60)
    k.ws.terminate()
    await new Promise((r) => setTimeout(r, 50))
    clock += 60_000
    expect((await beat()).body.credited).toBe(0)
  })
})
