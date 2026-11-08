// orbit.map çizimleri (canvas → doku): çekirdek HUD'u, oturum rozeti, istasyon simgeleri. Hepsi bizim çizimimiz;
// renkler palette.ts'ten (harita temadan bağımsız).
import * as THREE from 'three'
import type { Category, Status } from '../types'

const FONT = '"SF Mono", "JetBrains Mono", Menlo, monospace'

function texture(c: HTMLCanvasElement): THREE.CanvasTexture {
  const t = new THREE.CanvasTexture(c)
  t.colorSpace = THREE.SRGBColorSpace
  t.anisotropy = 4
  return t
}

// ───────────────────────────── çekirdek ─────────────────────────────

export interface CoreRing {
  /** 0–1 */
  value: number
  color: string
  label: string
}

export interface CoreState {
  /** Ortadaki büyük sayı */
  value: string
  /** Sayının altındaki küçük yazı */
  label: string
  rings: CoreRing[]
}

/** Çekirdek: iç içe ilerleme halkaları, ortada değer. Dokuyu yerinde yeniden çizer. */
export class CoreHud {
  readonly canvas = document.createElement('canvas')
  readonly texture: THREE.CanvasTexture
  private key = ''

  constructor() {
    this.canvas.width = this.canvas.height = 512
    this.texture = texture(this.canvas)
  }

  draw(s: CoreState) {
    const key = JSON.stringify(s)
    if (key === this.key) return
    this.key = key
    const g = this.canvas.getContext('2d')!
    const c = 256
    g.clearRect(0, 0, 512, 512)

    // koyu disk (arkasındaki yörüngeler soluk görünsün)
    const disk = g.createRadialGradient(c, c, 60, c, c, 250)
    disk.addColorStop(0, 'rgba(6,5,10,0.96)')
    disk.addColorStop(0.75, 'rgba(6,5,10,0.85)')
    disk.addColorStop(1, 'rgba(6,5,10,0)')
    g.fillStyle = disk
    g.beginPath()
    g.arc(c, c, 250, 0, Math.PI * 2)
    g.fill()

    // halkalar: dıştan içe
    s.rings.forEach((r, i) => {
      const radius = 214 - i * 30
      g.lineWidth = 12
      g.lineCap = 'round'
      g.strokeStyle = 'rgba(255,255,255,0.07)'
      g.beginPath()
      g.arc(c, c, radius, 0, Math.PI * 2)
      g.stroke()
      const v = Math.max(0, Math.min(1, r.value))
      if (v > 0.002) {
        g.strokeStyle = r.color
        g.shadowColor = r.color
        g.shadowBlur = 18
        g.beginPath()
        g.arc(c, c, radius, -Math.PI / 2, -Math.PI / 2 + v * Math.PI * 2)
        g.stroke()
        g.shadowBlur = 0
      }
    })

    // ince dış çerçeve + tik işaretleri
    g.strokeStyle = 'rgba(255,255,255,0.18)'
    g.lineWidth = 2
    g.beginPath()
    g.arc(c, c, 236, 0, Math.PI * 2)
    g.stroke()
    for (let i = 0; i < 48; i++) {
      const a = (i / 48) * Math.PI * 2
      const long = i % 4 === 0
      g.strokeStyle = long ? 'rgba(255,255,255,0.35)' : 'rgba(255,255,255,0.12)'
      g.beginPath()
      g.moveTo(c + Math.cos(a) * 236, c + Math.sin(a) * 236)
      g.lineTo(c + Math.cos(a) * (long ? 248 : 243), c + Math.sin(a) * (long ? 248 : 243))
      g.stroke()
    }

    // değer
    g.fillStyle = '#f5ebe0'
    g.textAlign = 'center'
    g.textBaseline = 'middle'
    g.shadowColor = 'rgba(255,176,87,0.8)'
    g.shadowBlur = 16
    g.font = `bold 84px ${FONT}`
    g.fillText(s.value, c, c - 12)
    g.shadowBlur = 0
    g.font = `bold 22px ${FONT}`
    g.fillStyle = 'rgba(245,235,224,0.6)'
    g.fillText(s.label.toUpperCase(), c, c + 48)

    // halka açıklamaları (küçük, altta)
    g.font = `16px ${FONT}`
    const legend = s.rings.map((r) => r.label).join(' · ')
    g.fillStyle = 'rgba(245,235,224,0.38)'
    g.fillText(legend, c, c + 78)
    this.texture.needsUpdate = true
  }

  dispose() {
    this.texture.dispose()
  }
}

// ───────────────────────────── oturum rozeti ─────────────────────────────

const STATUS_TEXT: Record<Status, string> = { busy: 'WORKING', waiting: 'NEEDS YOU', idle: 'YOUR TURN', ended: 'ENDED' }

/** Altıgen rozet: içinde proje adı ve durum, çevresinde ince halka. */
export function sessionBadge(project: string, status: Status, hue: string, statusColor: string): THREE.CanvasTexture {
  const c = document.createElement('canvas')
  c.width = c.height = 256
  const g = c.getContext('2d')!
  const cx = 128
  const cy = 128
  const hex = (r: number) => {
    g.beginPath()
    for (let i = 0; i < 6; i++) {
      const a = (Math.PI / 3) * i + Math.PI / 6
      const x = cx + Math.cos(a) * r
      const y = cy + Math.sin(a) * r
      if (i === 0) g.moveTo(x, y)
      else g.lineTo(x, y)
    }
    g.closePath()
  }
  // dış halka (oturumun rengi)
  g.strokeStyle = hue
  g.globalAlpha = 0.55
  g.lineWidth = 3
  g.shadowColor = hue
  g.shadowBlur = 14
  g.beginPath()
  g.arc(cx, cy, 122, 0, Math.PI * 2)
  g.stroke()
  g.globalAlpha = 1

  // altıgen: koyu dolgu, durum renginde parlak kenar (rozetin çoğunu kaplar: yazı okunsun)
  hex(108)
  const fill = g.createRadialGradient(cx, cy, 10, cx, cy, 90)
  fill.addColorStop(0, 'rgba(30,22,16,0.95)')
  fill.addColorStop(1, 'rgba(10,8,12,0.95)')
  g.fillStyle = fill
  g.shadowBlur = 0
  g.fill()
  g.strokeStyle = statusColor
  g.lineWidth = 6
  g.shadowColor = statusColor
  g.shadowBlur = 20
  g.stroke()
  g.shadowBlur = 0

  // yazılar
  g.textAlign = 'center'
  g.textBaseline = 'middle'
  g.fillStyle = '#f5ebe0'
  let size = 40
  const name = project.length > 12 ? `${project.slice(0, 11)}…` : project
  g.font = `bold ${size}px ${FONT}`
  while (g.measureText(name).width > 176 && size > 22) {
    size -= 2
    g.font = `bold ${size}px ${FONT}`
  }
  g.shadowColor = '#000'
  g.shadowBlur = 6
  g.fillText(name, cx, cy - 14)
  g.font = `bold 26px ${FONT}`
  g.fillStyle = statusColor
  g.shadowColor = statusColor
  g.shadowBlur = 10
  g.fillText(STATUS_TEXT[status], cx, cy + 28)
  g.shadowBlur = 0
  return texture(c)
}

// ───────────────────────────── istasyon simgeleri ─────────────────────────────

/** Çizgi simge (40x40 kutuda, merkez 0,0) */
function icon(g: CanvasRenderingContext2D, cat: Category) {
  g.beginPath()
  switch (cat) {
    case 'web': // küre
      g.arc(0, 0, 16, 0, Math.PI * 2)
      g.moveTo(-16, 0)
      g.lineTo(16, 0)
      g.ellipse(0, 0, 7, 16, 0, 0, Math.PI * 2)
      break
    case 'browser': // pencere + imleç
      g.rect(-17, -13, 34, 26)
      g.moveTo(-17, -6)
      g.lineTo(17, -6)
      g.moveTo(-2, 0)
      g.lineTo(-2, 11)
      g.lineTo(1, 8)
      g.lineTo(5, 12)
      break
    case 'read': // belge
      g.moveTo(-11, -16)
      g.lineTo(6, -16)
      g.lineTo(12, -10)
      g.lineTo(12, 16)
      g.lineTo(-11, 16)
      g.closePath()
      g.moveTo(-6, -3)
      g.lineTo(7, -3)
      g.moveTo(-6, 4)
      g.lineTo(7, 4)
      g.moveTo(-6, 10)
      g.lineTo(3, 10)
      break
    case 'search': // büyüteç
      g.arc(-4, -4, 10, 0, Math.PI * 2)
      g.moveTo(3, 3)
      g.lineTo(14, 14)
      break
    case 'edit': // kalem
      g.moveTo(-13, 13)
      g.lineTo(-10, 4)
      g.lineTo(8, -14)
      g.lineTo(14, -8)
      g.lineTo(-4, 10)
      g.closePath()
      g.moveTo(4, -10)
      g.lineTo(10, -4)
      break
    case 'bash': // >_
      g.moveTo(-14, -9)
      g.lineTo(-4, 0)
      g.lineTo(-14, 9)
      g.moveTo(1, 11)
      g.lineTo(14, 11)
      break
    case 'agent': // altıgen içinde nokta (alt ajan)
      for (let i = 0; i < 6; i++) {
        const a = (Math.PI / 3) * i + Math.PI / 6
        if (i === 0) g.moveTo(Math.cos(a) * 16, Math.sin(a) * 16)
        else g.lineTo(Math.cos(a) * 16, Math.sin(a) * 16)
      }
      g.closePath()
      g.moveTo(5, 0)
      g.arc(0, 0, 5, 0, Math.PI * 2)
      break
    case 'mcp': // fiş
      g.moveTo(-6, -16)
      g.lineTo(-6, -8)
      g.moveTo(6, -16)
      g.lineTo(6, -8)
      g.rect(-11, -8, 22, 12)
      g.moveTo(0, 4)
      g.lineTo(0, 16)
      break
    case 'plan': // liste
      g.moveTo(-8, -10)
      g.lineTo(14, -10)
      g.moveTo(-8, 0)
      g.lineTo(14, 0)
      g.moveTo(-8, 10)
      g.lineTo(14, 10)
      g.moveTo(-14, -10)
      g.arc(-14, -10, 1.5, 0, Math.PI * 2)
      g.moveTo(-14, 0)
      g.arc(-14, 0, 1.5, 0, Math.PI * 2)
      g.moveTo(-14, 10)
      g.arc(-14, 10, 1.5, 0, Math.PI * 2)
      break
    default:
      g.arc(0, 0, 10, 0, Math.PI * 2)
  }
  g.stroke()
}

/** İstasyon: parlayan küre + içinde kategori simgesi */
export function stationTexture(cat: Category, color: string): THREE.CanvasTexture {
  const c = document.createElement('canvas')
  c.width = c.height = 160
  const g = c.getContext('2d')!
  const cx = 80
  const glow = g.createRadialGradient(cx, cx, 8, cx, cx, 78)
  glow.addColorStop(0, color)
  glow.addColorStop(0.35, `${color}88`)
  glow.addColorStop(0.7, `${color}22`)
  glow.addColorStop(1, `${color}00`)
  g.fillStyle = glow
  g.fillRect(0, 0, 160, 160)
  // çekirdek küre
  g.fillStyle = 'rgba(8,6,12,0.82)'
  g.beginPath()
  g.arc(cx, cx, 30, 0, Math.PI * 2)
  g.fill()
  g.strokeStyle = color
  g.lineWidth = 3
  g.shadowColor = color
  g.shadowBlur = 12
  g.stroke()
  // simge
  g.save()
  g.translate(cx, cx)
  g.scale(0.85, 0.85)
  g.lineWidth = 3.4
  g.lineCap = 'round'
  g.lineJoin = 'round'
  g.strokeStyle = '#ffffff'
  g.shadowColor = color
  g.shadowBlur = 8
  icon(g, cat)
  g.restore()
  return texture(c)
}

/** İstasyonların görünen adları (referanstaki gibi `alan.eylem`) */
export const STATION_NAME: Record<Category, string> = {
  web: 'web.research',
  browser: 'browser.drive',
  read: 'files.read',
  search: 'code.search',
  edit: 'files.write',
  bash: 'shell.exec',
  agent: 'agents.spawn',
  mcp: 'mcp.call',
  plan: 'plan.skill',
  other: 'tools.other',
}

// ───────────────────────────── terminal rozeti ─────────────────────────────

/** Terminal: iç içe halkalar + ortada altıgen; içinde ad ve durum (RUNNING / TERMINAL / LEFTOVER). */
export function terminalBadge(name: string, state: string, color: string): THREE.CanvasTexture {
  const c = document.createElement('canvas')
  c.width = c.height = 256
  const g = c.getContext('2d')!
  const cx = 128
  // iç içe halkalar (dışa doğru solar)
  g.strokeStyle = color
  g.shadowColor = color
  g.shadowBlur = 16
  ;[
    [122, 3, 0.35],
    [110, 5, 0.85],
    [98, 2, 0.5],
  ].forEach(([r, w, a]) => {
    g.globalAlpha = a
    g.lineWidth = w
    g.beginPath()
    g.arc(cx, cx, r, 0, Math.PI * 2)
    g.stroke()
  })
  g.globalAlpha = 1
  // altıgen
  g.beginPath()
  for (let i = 0; i < 6; i++) {
    const a = (Math.PI / 3) * i + Math.PI / 6
    const x = cx + Math.cos(a) * 82
    const y = cx + Math.sin(a) * 82
    if (i === 0) g.moveTo(x, y)
    else g.lineTo(x, y)
  }
  g.closePath()
  g.fillStyle = 'rgba(14,10,8,0.96)'
  g.shadowBlur = 0
  g.fill()
  g.lineWidth = 4
  g.shadowBlur = 12
  g.stroke()
  g.shadowBlur = 0
  // yazılar
  g.textAlign = 'center'
  g.textBaseline = 'middle'
  g.fillStyle = '#f5ebe0'
  let size = 34
  const label = name.length > 11 ? `${name.slice(0, 10)}…` : name
  g.font = `bold ${size}px ${FONT}`
  while (g.measureText(label).width > 136 && size > 18) {
    size -= 2
    g.font = `bold ${size}px ${FONT}`
  }
  g.fillText(label, cx, cx - 12)
  g.font = `bold 20px ${FONT}`
  g.fillStyle = color
  g.fillText(state, cx, cx + 22)
  return texture(c)
}
