// Canlı bağlantılar (WS /ws): kim çevrimiçi, kim AFK; arkadaşlara durum, yeni mesaj ve istek olayları.
// Bellekte tutulur (tek sunucu süreci). "is now online" bildirimi yalnızca 10+ dk yokluktan sonra.
import type { WebSocket } from '@fastify/websocket'

/** "is now online" için en az bu kadar yokluk */
export const LONG_AWAY_MS = 10 * 60_000

export type LiveEvent =
  | { type: 'presence'; name: string; tag: string; online: boolean; away: boolean; notify: boolean }
  | {
      type: 'message'
      id: string
      from: { name: string; tag: string }
      /** Şifreli metin (hex); istemci kendi gizli anahtarıyla çözer */
      ciphertext: string
      nonce: string
      fromKey: string
      toKey: string
      at: number
    }
  | { type: 'request'; from: { name: string; tag: string } }
  | { type: 'friend'; name: string; tag: string; status: 'accepted' | 'removed' }
  | { type: 'hello'; name: string; tag: string }
  | { type: 'announcement'; id: string; title: string; body: string; at: number }

export class Live {
  private sockets = new Map<string, Set<WebSocket>>()
  private lastOffline = new Map<string, number>()
  private awayFlag = new Map<string, boolean>()

  isOnline(id: string): boolean {
    return (this.sockets.get(id)?.size ?? 0) > 0
  }

  isAway(id: string): boolean {
    return this.awayFlag.get(id) ?? false
  }

  setAway(id: string, away: boolean): boolean {
    const changed = this.isAway(id) !== away
    this.awayFlag.set(id, away)
    return changed
  }

  /** Bağlantı eklendi. Uzun bir yokluktan sonra ilk bağlantıysa true ("is now online" bildirilir). */
  add(id: string, ws: WebSocket, now: number): { first: boolean; longAway: boolean } {
    let set = this.sockets.get(id)
    const first = !set || set.size === 0
    if (!set) this.sockets.set(id, (set = new Set()))
    set.add(ws)
    const off = this.lastOffline.get(id)
    return { first, longAway: first && (off === undefined || now - off >= LONG_AWAY_MS) }
  }

  /** Bağlantı kapandı. Son bağlantıysa true (çevrimdışı oldu). */
  remove(id: string, ws: WebSocket, now: number): boolean {
    const set = this.sockets.get(id)
    if (!set) return false
    set.delete(ws)
    if (set.size > 0) return false
    this.sockets.delete(id)
    this.awayFlag.delete(id)
    this.lastOffline.set(id, now)
    return true
  }

  send(id: string, ev: LiveEvent): void {
    const msg = JSON.stringify(ev)
    for (const ws of this.sockets.get(id) ?? []) {
      if (ws.readyState === 1) ws.send(msg)
    }
  }

  /** Bağlı herkese (duyurular) */
  broadcast(ev: LiveEvent): void {
    for (const id of this.sockets.keys()) this.send(id, ev)
  }

  closeAll(): void {
    for (const set of this.sockets.values()) for (const ws of set) ws.close()
    this.sockets.clear()
  }
}
