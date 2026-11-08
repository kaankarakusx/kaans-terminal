import { createHash, randomBytes, randomInt } from 'node:crypto'

export const sha256 = (s: string) => createHash('sha256').update(s).digest('hex')

/** Cihaz token'ı: 32 rastgele bayt. Sunucuda yalnızca SHA-256'sı saklanır. */
export const newToken = () => randomBytes(32).toString('base64url')

/** 4 haneli tag (0001–9999) */
export const randomTag = () => String(randomInt(1, 10000)).padStart(4, '0')

/** Hacker name: 3–20 karakter, harf/rakam/_ . - */
export const NAME_RE = /^[A-Za-z0-9_.-]{3,20}$/

/** Saat dilimi (UTC'ye göre dakika, doğuya artı) ile yerel gün */
export function localDay(nowMs: number, tzMin: number): string {
  return new Date(nowMs + tzMin * 60_000).toISOString().slice(0, 10)
}

/** Kullanıcıya gösterilecek hata (durum kodu + İngilizce mesaj) */
export class HttpError extends Error {
  status: number
  constructor(status: number, message: string) {
    super(message)
    this.status = status
  }
}
