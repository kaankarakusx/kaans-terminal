// orbit.map: three.js sahnesi (TASARIM.md §6). Svelte'ten bağımsız; OrbitMap.svelte sarmalar.
//
// - Ortada HUD çekirdeği: iç içe ilerleme halkaları (cache hit, limitler, etkinlik) ve tools/min; arkasındakini gizler.
// - Her şey tek bir yatay düzlemde: oturumlar 3 düzenli halkada döner (rozet: proje + durum), araç istasyonları
//   dış halkada (parlayan küre + simge, HTML etiket: `files.read 248`, altında son hedef). Kamera hafif yukarıdan bakar.
// - Seni bekleyen oturum yörüngeden çıkıp görünümün kenarına park eder (ilk aldığı yer onundur).
// - Alt ajanlar oturumun çevresinde ay gibi döner; araç çağrıları dış halkadaki istasyonlara ışık oku olarak uçar.
import * as THREE from 'three'
import { EffectComposer } from 'three/addons/postprocessing/EffectComposer.js'
import { OutputPass } from 'three/addons/postprocessing/OutputPass.js'
import { RenderPass } from 'three/addons/postprocessing/RenderPass.js'
import { UnrealBloomPass } from 'three/addons/postprocessing/UnrealBloomPass.js'
import { RoomEnvironment } from 'three/addons/environments/RoomEnvironment.js'
import type { Category, Status } from '../types'
import { reportError } from '../errors'
import { CoreHud, STATION_NAME, sessionBadge, stationTexture, type CoreState } from './hud'
import { CATEGORY, CORE, CORE_HOT, ERROR, HUES, SPACE, STATUS, SUBAGENT } from './palette'
import { Vehicles, type AppInput, type CarInput, type MeteorInput, type Pick } from './vehicles'

export type Hit = { kind: 'session' } & { id: string } | Pick

export interface OrbitSession {
  id: string
  title: string
  /** Rozetin içinde yazan proje adı */
  project: string
  status: Status
  hue: number
  subagents: number
}

/** İstasyon etiketi: bugünkü çağrı sayısı ve son hedef */
export interface StationInfo {
  count: number
  last: string
}

export type PowerMode = 'foreground' | 'background' | 'hidden'

export interface OrbitOptions {
  onSelect: (hit: Hit) => void
  onHover: (hit: (Hit & { x: number; y: number }) | null) => void
  /** Kullanıcı kamerayı ellediğinde (sinematik kamerayı bitirmek için). */
  onUserCamera: () => void
  reducedMotion: boolean
}

const STATIONS: Category[] = ['web', 'browser', 'read', 'search', 'edit', 'bash', 'agent', 'mcp', 'plan']
const STATION_RADIUS = 10
/** Oturum halkaları (çekirdekten dışa) */
const RINGS = [4.8, 6.9]
/** Rozetin dünya boyutu */
const BADGE = 2.2
/** Çekirdek HUD'unun dünya boyutu ve gizleme yarıçapı */
const CORE_SIZE = 4.4
const CORE_OCCLUDE = 1.9
const HOME = { r: 27, theta: 0, phi: Math.PI / 2 - 0.44 }
const PARK_DEPTH = 14
const BOLT_POINTS = 14
const MAX_BOLTS = 60

const ease = (t: number) => (t < 0.5 ? 4 * t * t * t : 1 - (-2 * t + 2) ** 3 / 2)

function hash(s: string): number {
  let h = 2166136261
  for (let i = 0; i < s.length; i++) h = Math.imul(h ^ s.charCodeAt(i), 16777619)
  return (h >>> 0) / 4294967295
}

function hexPoints(r: number): THREE.Vector3[] {
  return Array.from({ length: 6 }, (_, i) => {
    const a = (Math.PI / 3) * i + Math.PI / 6
    return new THREE.Vector3(Math.cos(a) * r, Math.sin(a) * r, 0)
  })
}

function glowTexture(): THREE.Texture {
  const c = document.createElement('canvas')
  c.width = c.height = 128
  const g = c.getContext('2d')!
  const grad = g.createRadialGradient(64, 64, 0, 64, 64, 64)
  grad.addColorStop(0, 'rgba(255,255,255,1)')
  grad.addColorStop(0.25, 'rgba(255,255,255,0.45)')
  grad.addColorStop(1, 'rgba(255,255,255,0)')
  g.fillStyle = grad
  g.fillRect(0, 0, 128, 128)
  const t = new THREE.CanvasTexture(c)
  t.colorSpace = THREE.SRGBColorSpace
  return t
}

interface Node {
  data: OrbitSession
  group: THREE.Group
  /** Rozet (proje + durum); tıklama da buna */
  badge: THREE.Sprite
  badgeKey: string
  glow: THREE.Sprite
  slot: number
  angle: number
  speed: number
  radius: number
  /** 0 = yörüngede, 1 = park etmiş */
  park: number
  parkSlot: number | null
  moons: THREE.Group[]
  label: HTMLDivElement
  /** sürükleme / fırlatma: evden sapma ve hızı */
  offset: THREE.Vector3
  velocity: THREE.Vector3
  world: THREE.Vector3
  gone: number
}

interface Bolt {
  line: THREE.Line
  from: THREE.Vector3
  to: THREE.Vector3
  ctrl: THREE.Vector3
  t: number
  station: Category
  error: boolean
}

interface Flight {
  from: { r: number; theta: number; phi: number; target: THREE.Vector3 }
  to: { r: number; theta: number; phi: number; target: THREE.Vector3 }
  t: number
  dur: number
}

export class OrbitScene {
  private renderer: THREE.WebGLRenderer
  private composer: EffectComposer
  private bloom: UnrealBloomPass
  private scene = new THREE.Scene()
  private camera: THREE.PerspectiveCamera
  private core: THREE.Sprite
  private coreHud = new CoreHud()
  private coreGlow: THREE.Sprite
  private dust: THREE.Points
  private nodes = new Map<string, Node>()
  private stations = new Map<
    Category,
    { sprite: THREE.Sprite; pos: THREE.Vector3; flash: number; error: number; label: HTMLDivElement; count: HTMLElement; last: HTMLElement }
  >()
  private bolts: Bolt[] = []
  private glowTex = glowTexture()
  private labels: HTMLDivElement
  private raycaster = new THREE.Raycaster()
  private cam = { ...HOME, target: new THREE.Vector3() }
  private flight: Flight | null = null
  private cinematic = false
  private cineT = 0
  private dragging: { id: string | null; mode: 'session' | 'rotate' | 'pan'; x: number; y: number; moved: boolean; button: number } | null = null
  private parkSlots: (string | null)[] = []
  private nextOrbitSlot = 0
  private activity = 0
  /** Halkaların ortak dönüşü (oturumlar halkada eşit aralıklı dizilir ve birlikte döner) */
  private spin = 0
  private mode: PowerMode = 'foreground'
  private saver = false
  private pixelRatio = 1
  private lowerAt = 0
  private raf = 0
  private last = performance.now()
  private lastFrame = 0
  private time = 0
  private disposed = false
  private ro: ResizeObserver
  private minimap: HTMLCanvasElement | null = null
  private frame = 0
  private tmpDir = new THREE.Vector3()
  private pressed: Hit | null = null
  private vehicles: Vehicles
  private jolt = new THREE.Vector3()
  /** Meteor takibinden önceki kamera hedefi (takip bitince oraya dönülür) */
  private beforeFollow: THREE.Vector3 | null = null

  constructor(
    private host: HTMLElement,
    private opts: OrbitOptions,
  ) {
    this.renderer = new THREE.WebGLRenderer({ antialias: false, powerPreference: 'high-performance' })
    this.renderer.setClearColor(SPACE)
    this.renderer.domElement.className = 'orbit-canvas'
    host.appendChild(this.renderer.domElement)

    this.labels = document.createElement('div')
    this.labels.className = 'orbit-labels'
    host.appendChild(this.labels)

    this.camera = new THREE.PerspectiveCamera(42, 1, 0.1, 600)
    this.scene.fog = null

    // Çekirdek: kameraya dönük HUD (koyu diski derinlik yazar: arkasından geçeni gizler) + yumuşak parıltı.
    this.coreHud.draw({ value: '0.0', label: 'tools/min', rings: [] })
    this.core = new THREE.Sprite(
      new THREE.SpriteMaterial({ map: this.coreHud.texture, transparent: true, alphaTest: 0.35, depthWrite: true }),
    )
    this.core.scale.setScalar(CORE_SIZE)
    this.core.renderOrder = 2
    this.coreGlow = new THREE.Sprite(
      new THREE.SpriteMaterial({ map: this.glowTex, color: CORE_HOT, transparent: true, blending: THREE.AdditiveBlending, depthWrite: false }),
    )
    this.coreGlow.scale.setScalar(6)
    this.scene.add(this.coreGlow, this.core)
    this.buildRings()

    // Arabaların boyası ve camı ortamı yansıtsın.
    const pmrem = new THREE.PMREMGenerator(this.renderer)
    this.scene.environment = pmrem.fromScene(new RoomEnvironment(), 0.04).texture
    pmrem.dispose()
    const sun = new THREE.DirectionalLight('#ffd9a8', 1.4)
    sun.position.set(6, 10, 8)
    this.scene.add(sun)
    this.vehicles = new Vehicles(this.scene, this.camera, this.glowTex, opts.reducedMotion)
    this.vehicles.setLabelLayer(this.labels, host)

    this.scene.add(this.stars(1300, 140, 260, 0.8, 0.6))
    this.dust = this.stars(320, 16, 50, 0.3, 0.14)
    this.scene.add(this.dust)
    this.buildStations()

    this.composer = new EffectComposer(this.renderer)
    this.composer.addPass(new RenderPass(this.scene, this.camera))
    // Bloom yarı çözünürlükte; daha fazla küçültülmez, yoksa ince hareketli çizgiler titrer.
    this.bloom = new UnrealBloomPass(new THREE.Vector2(256, 256), 0.55, 0.4, 0.35)
    this.composer.addPass(this.bloom)
    this.composer.addPass(new OutputPass())

    this.bindInput()
    this.ro = new ResizeObserver(() => this.resize())
    this.ro.observe(host)
    this.resize()
    this.loop()
  }

  // ───────────────────────────── kurulum ─────────────────────────────

  private stars(n: number, rMin: number, rMax: number, size: number, opacity: number): THREE.Points {
    const pos = new Float32Array(n * 3)
    for (let i = 0; i < n; i++) {
      const v = new THREE.Vector3().randomDirection().multiplyScalar(rMin + Math.random() * (rMax - rMin))
      pos.set([v.x, v.y, v.z], i * 3)
    }
    const geo = new THREE.BufferGeometry()
    geo.setAttribute('position', new THREE.BufferAttribute(pos, 3))
    return new THREE.Points(
      geo,
      new THREE.PointsMaterial({
        color: '#c9d2e6',
        size,
        map: this.glowTex,
        sizeAttenuation: true,
        transparent: true,
        opacity,
        depthWrite: false,
        blending: THREE.AdditiveBlending,
      }),
    )
  }

  /** Düzlemdeki soluk halkalar: oturum halkaları, çekirdeğin hemen çevresi, istasyon halkası */
  private buildRings() {
    const ring = (r: number, color: string, opacity: number, dashed = false) => {
      const pts = Array.from({ length: 200 }, (_, i) => {
        const a = (i / 200) * Math.PI * 2
        return new THREE.Vector3(Math.cos(a) * r, 0, Math.sin(a) * r)
      })
      const geo = new THREE.BufferGeometry().setFromPoints(pts)
      const mat = dashed
        ? new THREE.LineDashedMaterial({ color, transparent: true, opacity, dashSize: 0.35, gapSize: 0.35, depthWrite: false })
        : new THREE.LineBasicMaterial({ color, transparent: true, opacity, depthWrite: false })
      const line = new THREE.LineLoop(geo, mat)
      if (dashed) line.computeLineDistances()
      this.scene.add(line)
    }
    ring(2.55, '#5b8cff', 0.55)
    RINGS.forEach((r) => ring(r, '#c9d2e6', 0.09))
    ring(STATION_RADIUS, '#c9d2e6', 0.12, true)
  }

  private buildStations() {
    STATIONS.forEach((cat, i) => {
      const a = -Math.PI / 2 + (i / STATIONS.length) * Math.PI * 2
      const pos = new THREE.Vector3(Math.cos(a) * STATION_RADIUS, 0, Math.sin(a) * STATION_RADIUS)
      const sprite = new THREE.Sprite(
        new THREE.SpriteMaterial({ map: stationTexture(cat, CATEGORY[cat]), transparent: true, depthWrite: false, opacity: 0.85 }),
      )
      sprite.scale.setScalar(1.45)
      sprite.position.copy(pos)
      this.scene.add(sprite)
      // HTML etiket: ad (kategori renginde) + sayı + kare; altında son hedef
      const label = document.createElement('div')
      label.className = 'station-label'
      label.style.setProperty('--c', CATEGORY[cat])
      const head = document.createElement('div')
      head.className = 'st-head'
      const name = document.createElement('b')
      name.textContent = STATION_NAME[cat]
      const count = document.createElement('span')
      count.className = 'st-count'
      count.textContent = '0'
      const sq = document.createElement('i')
      head.append(name, count, sq)
      const last = document.createElement('div')
      last.className = 'st-last'
      label.append(head, last)
      this.labels.appendChild(label)
      this.stations.set(cat, { sprite, pos, flash: 0, error: 0, label, count, last })
    })
  }

  private makeNode(s: OrbitSession): Node {
    const color = new THREE.Color(HUES[s.hue % HUES.length])
    const group = new THREE.Group()
    const glow = new THREE.Sprite(
      new THREE.SpriteMaterial({ map: this.glowTex, color, transparent: true, opacity: 0.22, blending: THREE.AdditiveBlending, depthWrite: false }),
    )
    glow.scale.setScalar(BADGE * 1.6)
    const badge = new THREE.Sprite(
      new THREE.SpriteMaterial({ map: sessionBadge(s.project, s.status, HUES[s.hue % HUES.length], STATUS[s.status]), transparent: true, depthWrite: false }),
    )
    badge.scale.setScalar(BADGE)
    badge.userData.id = s.id
    badge.renderOrder = 3
    group.add(glow, badge)
    this.scene.add(group)

    const slot = this.nextOrbitSlot++
    const label = document.createElement('div')
    label.className = 'orbit-label'
    label.style.color = HUES[s.hue % HUES.length]
    this.labels.appendChild(label)

    return {
      data: s, group, badge, badgeKey: `${s.project}|${s.status}|${s.hue}`, glow, slot,
      angle: hash(s.id) * Math.PI * 2,
      speed: 0.12 + hash(s.id + 'v') * 0.08,
      radius: RINGS[slot % RINGS.length], park: 0, parkSlot: null, moons: [], label,
      offset: new THREE.Vector3(), velocity: new THREE.Vector3(), world: new THREE.Vector3(), gone: 0,
    }
  }

  private setMoons(n: Node, count: number) {
    const want = Math.min(count, 6)
    while (n.moons.length < want) {
      const g = new THREE.Group()
      const fill = new THREE.Mesh(
        new THREE.CircleGeometry(0.15, 6),
        new THREE.MeshBasicMaterial({ color: new THREE.Color(SUBAGENT).multiplyScalar(0.25) }),
      )
      const edge = new THREE.LineLoop(
        new THREE.BufferGeometry().setFromPoints(hexPoints(0.15)),
        new THREE.LineBasicMaterial({ color: SUBAGENT }),
      )
      g.add(fill, edge)
      this.scene.add(g)
      n.moons.push(g)
    }
    while (n.moons.length > want) this.disposeObject(n.moons.pop()!)
  }

  // ───────────────────────────── dış API ─────────────────────────────

  setSessions(list: OrbitSession[]) {
    const ids = new Set(list.map((s) => s.id))
    for (const s of list) {
      let n = this.nodes.get(s.id)
      if (!n) {
        n = this.makeNode(s)
        this.nodes.set(s.id, n)
      }
      n.data = s
      const key = `${s.project}|${s.status}|${s.hue}`
      if (key !== n.badgeKey) {
        const mat = n.badge.material as THREE.SpriteMaterial
        mat.map?.dispose()
        mat.map = sessionBadge(s.project, s.status, HUES[s.hue % HUES.length], STATUS[s.status])
        mat.needsUpdate = true
        n.badgeKey = key
      }
      n.label.textContent = s.title
      this.setMoons(n, s.subagents)
    }
    for (const [id, n] of this.nodes) {
      if (!ids.has(id)) {
        this.freePark(n)
        this.disposeNode(n)
        this.nodes.delete(id)
      }
    }
  }

  /** Çekirdek HUD'u (değer, halkalar) */
  setCore(state: CoreState) {
    this.coreHud.draw(state)
  }

  /** İstasyon etiketleri: bugünkü sayılar ve son hedefler */
  setStations(info: Partial<Record<Category, StationInfo>>) {
    for (const [cat, st] of this.stations) {
      const i = info[cat]
      st.count.textContent = String(i?.count ?? 0)
      st.last.textContent = i?.last ?? ''
    }
  }

  /** Terminaller (araba) ve kapanmış terminallerden kalan süreçler (amber araba). */
  setCars(list: CarInput[]) {
    this.vehicles.setCars(list)
  }

  /** Çalışan uygulamalar (dev sunucuları): arabalar */
  setApps(list: AppInput[]) {
    this.vehicles.setApps(list)
  }

  /** Kaçak süreçler (meteor). */
  setMeteors(list: MeteorInput[]) {
    this.vehicles.setMeteors(list)
  }

  /** End Session ile kapanan terminalin arabası patlar. */
  explode(id: string) {
    this.vehicles.explode(id)
  }

  /** Araç çağrısı: oturumdan istasyona ışık oku. */
  bolt(sessionId: string, category: Category | null, error = false) {
    const n = this.nodes.get(sessionId)
    const st = this.stations.get(category && this.stations.has(category) ? category : 'plan')
    if (!n || !st) return
    this.activity = Math.min(1, this.activity + 0.08)
    if (error) {
      st.error = 1
      return
    }
    if (this.bolts.length >= MAX_BOLTS) return
    const from = n.world.clone()
    const to = st.pos.clone()
    const ctrl = from.clone().add(to).multiplyScalar(0.5)
    ctrl.add(ctrl.clone().setY(0).normalize().multiplyScalar(1.5)).add(new THREE.Vector3(0, 3, 0))
    const geo = new THREE.BufferGeometry()
    geo.setAttribute('position', new THREE.BufferAttribute(new Float32Array(BOLT_POINTS * 3), 3))
    const colors = new Float32Array(BOLT_POINTS * 3)
    const c = new THREE.Color(CATEGORY[category ?? 'other'])
    for (let i = 0; i < BOLT_POINTS; i++) {
      const k = i / (BOLT_POINTS - 1)
      colors.set([c.r * k, c.g * k, c.b * k], i * 3)
    }
    geo.setAttribute('color', new THREE.BufferAttribute(colors, 3))
    const line = new THREE.Line(geo, new THREE.LineBasicMaterial({ vertexColors: true, transparent: true, blending: THREE.AdditiveBlending, depthWrite: false }))
    this.scene.add(line)
    this.bolts.push({ line, from, to, ctrl, t: 0, station: category ?? 'plan', error })
  }

  setPower(mode: PowerMode, saver: boolean) {
    this.mode = mode
    this.saver = saver
  }

  setMinimap(canvas: HTMLCanvasElement | null) {
    this.minimap = canvas
  }

  zoom(factor: number) {
    this.flyTo({ r: THREE.MathUtils.clamp(this.cam.r * factor, 8, 70) }, 0.35)
  }

  reset() {
    this.flyTo({ ...HOME, target: new THREE.Vector3() }, 0.9)
  }

  /** Minimap tıklaması: o yönden bakan bir görünüme uç. */
  lookFrom(x: number, z: number) {
    this.flyTo({ theta: Math.atan2(x, z) }, 0.9)
  }

  setCinematic(on: boolean) {
    if (on === this.cinematic) return
    this.cinematic = on
    this.cineT = 0
    if (!on) this.flyTo({ ...HOME, target: new THREE.Vector3() }, 1.2)
  }

  get isCinematic() {
    return this.cinematic
  }

  dispose() {
    this.coreHud.dispose()
    this.disposed = true
    cancelAnimationFrame(this.raf)
    this.ro.disconnect()
    for (const n of this.nodes.values()) this.disposeNode(n)
    for (const b of this.bolts) this.disposeObject(b.line)
    this.vehicles.dispose()
    this.scene.traverse((o) => {
      const m = o as THREE.Mesh
      m.geometry?.dispose()
      const mat = m.material as THREE.Material | THREE.Material[] | undefined
      if (Array.isArray(mat)) mat.forEach((x) => x.dispose())
      else mat?.dispose()
    })
    this.glowTex.dispose()
    this.composer.dispose()
    this.renderer.dispose()
    this.renderer.domElement.remove()
    this.labels.remove()
  }

  // ───────────────────────────── kamera ─────────────────────────────

  private flyTo(to: Partial<typeof HOME & { target: THREE.Vector3 }>, dur: number) {
    const from = { ...this.cam, target: this.cam.target.clone() }
    const dest = { ...from, ...to, target: (to.target ?? from.target).clone() }
    // Kısa yoldan dön (küresel koordinatlarda; sahnenin içinden düz geçilmez).
    let d = dest.theta - from.theta
    d = ((d + Math.PI) % (Math.PI * 2) + Math.PI * 2) % (Math.PI * 2) - Math.PI
    dest.theta = from.theta + d
    this.flight = { from, to: dest, t: 0, dur: this.opts.reducedMotion ? 0.01 : dur }
  }

  private applyCamera() {
    const { r, theta, phi, target } = this.cam
    this.camera.position.set(
      target.x + r * Math.sin(phi) * Math.sin(theta),
      target.y + r * Math.cos(phi),
      target.z + r * Math.sin(phi) * Math.cos(theta),
    )
    this.camera.lookAt(target)
    // Patlama sarsıntısı
    this.camera.position.add(this.vehicles.joltOffset(this.jolt))
  }

  // ───────────────────────────── giriş ─────────────────────────────

  private ndc(e: PointerEvent | MouseEvent) {
    const r = this.renderer.domElement.getBoundingClientRect()
    return new THREE.Vector2(((e.clientX - r.left) / r.width) * 2 - 1, -((e.clientY - r.top) / r.height) * 2 + 1)
  }

  private pick(e: PointerEvent | MouseEvent): Hit | null {
    this.raycaster.setFromCamera(this.ndc(e), this.camera)
    const fills = [...this.nodes.values()].map((n) => n.badge)
    const hit = this.raycaster.intersectObjects([...fills, ...this.vehicles.pickables()], false)[0]
    if (!hit) return null
    const d = hit.object.userData as { id?: string; kind?: Pick['kind'] }
    if (!d.id) return null
    return d.kind ? { kind: d.kind, id: d.id } : { kind: 'session', id: d.id }
  }

  private bindInput() {
    const el = this.renderer.domElement
    el.addEventListener('contextmenu', (e) => e.preventDefault())
    el.addEventListener('pointerdown', (e) => {
      el.setPointerCapture(e.pointerId)
      const hit = e.button === 0 ? this.pick(e) : null
      this.pressed = hit
      // Yalnızca oturumlar sürüklenir; araba/meteor tıklanır.
      const id = hit?.kind === 'session' ? hit.id : null
      const mode = hit ? 'session' : e.button === 2 || e.shiftKey ? 'pan' : 'rotate'
      this.dragging = { id, mode, x: e.clientX, y: e.clientY, moved: false, button: e.button }
      if (mode !== 'session') {
        this.flight = null
        this.opts.onUserCamera()
      }
    })
    el.addEventListener('pointermove', (e) => {
      const d = this.dragging
      if (!d) {
        const hit = this.pick(e)
        const r = this.host.getBoundingClientRect()
        this.opts.onHover(hit ? { ...hit, x: e.clientX - r.left, y: e.clientY - r.top } : null)
        return
      }
      const dx = e.clientX - d.x
      const dy = e.clientY - d.y
      if (!d.moved && Math.hypot(dx, dy) < 4) return
      d.moved = true
      d.x = e.clientX
      d.y = e.clientY
      this.opts.onHover(null)
      if (d.mode === 'rotate') {
        this.cam.theta -= dx * 0.006
        this.cam.phi = THREE.MathUtils.clamp(this.cam.phi - dy * 0.006, 0.15, Math.PI - 0.15)
      } else if (d.mode === 'pan') {
        const k = this.cam.r * 0.0016
        const right = new THREE.Vector3().setFromMatrixColumn(this.camera.matrix, 0)
        const up = new THREE.Vector3().setFromMatrixColumn(this.camera.matrix, 1)
        this.cam.target.addScaledVector(right, -dx * k).addScaledVector(up, dy * k)
      } else if (d.id) {
        // Oturumu kameraya dik düzlemde sürükle; bırakınca evine yaylanarak döner (fırlatılabilir).
        const n = this.nodes.get(d.id)
        if (!n) return
        const k = this.camera.position.distanceTo(n.world) * 0.0016
        const right = new THREE.Vector3().setFromMatrixColumn(this.camera.matrix, 0)
        const up = new THREE.Vector3().setFromMatrixColumn(this.camera.matrix, 1)
        const delta = right.multiplyScalar(dx * k).addScaledVector(up, -dy * k)
        n.offset.add(delta)
        n.velocity.copy(delta).multiplyScalar(60)
      }
    })
    const end = () => {
      const d = this.dragging
      this.dragging = null
      if (d?.mode === 'session' && !d.moved && this.pressed) this.opts.onSelect(this.pressed)
      this.pressed = null
    }
    el.addEventListener('pointerup', end)
    el.addEventListener('pointercancel', end)
    el.addEventListener('pointerleave', () => !this.dragging && this.opts.onHover(null))
    el.addEventListener(
      'wheel',
      (e) => {
        e.preventDefault()
        this.flight = null
        this.opts.onUserCamera()
        this.cam.r = THREE.MathUtils.clamp(this.cam.r * Math.exp(e.deltaY * 0.0012), 8, 70)
      },
      { passive: false },
    )
    el.addEventListener('dblclick', () => {
      this.opts.onUserCamera()
      this.reset()
    })
  }

  // ───────────────────────────── park etme ─────────────────────────────

  private takePark(n: Node) {
    if (n.parkSlot !== null) return
    let i = this.parkSlots.indexOf(null)
    if (i < 0) i = this.parkSlots.push(null) - 1
    this.parkSlots[i] = n.data.id
    n.parkSlot = i
  }

  private freePark(n: Node) {
    if (n.parkSlot === null) return
    this.parkSlots[n.parkSlot] = null
    n.parkSlot = null
  }

  /** Park yeri ekran uzayında sabittir: sağ kenar yukarıdan aşağı, dolunca sol kenar. */
  private parkPosition(slot: number, out: THREE.Vector3) {
    const perCol = 5
    const col = Math.floor(slot / perCol)
    const row = slot % perCol
    const x = col % 2 === 0 ? 0.86 : -0.86
    const y = 0.72 - row * 0.3
    // Not: yön ayrı bir vektörde tutulur; `out` hem yön hem sonuç olursa kamera konumu yönü siler.
    const dir = this.tmpDir.set(x, y, 0.5).unproject(this.camera).sub(this.camera.position).normalize()
    return out.copy(this.camera.position).addScaledVector(dir, PARK_DEPTH)
  }

  // ───────────────────────────── döngü ─────────────────────────────

  private resize() {
    const w = Math.max(1, this.host.clientWidth)
    const h = Math.max(1, this.host.clientHeight)
    this.camera.aspect = w / h
    this.camera.updateProjectionMatrix()
    this.renderer.setPixelRatio(this.pixelRatio)
    this.renderer.setSize(w, h)
    this.composer.setPixelRatio(this.pixelRatio)
    this.composer.setSize(w, h)
    this.bloom.setSize((w * this.pixelRatio) / 2, (h * this.pixelRatio) / 2)
  }

  /** Güç moduna göre çözünürlük: hemen keskinleşir, düşmesi ancak 3 sn öyle kaldıktan sonra olur. */
  private updatePixelRatio(now: number, interacting: boolean) {
    const dpr = Math.min(window.devicePixelRatio || 1, 2)
    let want = this.mode === 'foreground' || interacting ? dpr : 1
    if (this.saver) want = Math.min(want, 1.5)
    if (want > this.pixelRatio) {
      this.pixelRatio = want
      this.lowerAt = 0
      this.resize()
    } else if (want < this.pixelRatio) {
      if (!this.lowerAt) this.lowerAt = now + 3000
      else if (now >= this.lowerAt) {
        this.pixelRatio = want
        this.lowerAt = 0
        this.resize()
      }
    } else this.lowerAt = 0
  }

  private loop = () => {
    if (this.disposed) return
    this.raf = requestAnimationFrame(this.loop)
    const now = performance.now()
    if (this.mode === 'hidden' || document.hidden) {
      this.last = now
      return
    }
    const interacting = !!this.dragging || !!this.flight
    let fps = this.mode === 'foreground' || interacting ? 60 : 20
    if (this.saver && !interacting) fps = Math.min(fps, 30)
    if (now - this.lastFrame < 1000 / fps - 2) return
    this.lastFrame = now
    const dt = Math.min(0.1, (now - this.last) / 1000)
    this.last = now
    // Bir karedeki hata sahneyi dondurmasın: günlüğe yazılır (aynı hata 10 sn'de bir), sonraki kare devam eder
    try {
      this.updatePixelRatio(now, interacting)
      this.update(dt)
      this.composer.render()
    } catch (e) {
      reportError(e, 'orbit.frame')
    }
  }

  private update(dt: number) {
    this.time += dt
    const slow = this.opts.reducedMotion ? 0.3 : 1

    // kamera
    if (this.flight) {
      const f = this.flight
      f.t = Math.min(1, f.t + dt / f.dur)
      const k = ease(f.t)
      this.cam.r = THREE.MathUtils.lerp(f.from.r, f.to.r, k)
      this.cam.theta = THREE.MathUtils.lerp(f.from.theta, f.to.theta, k)
      this.cam.phi = THREE.MathUtils.lerp(f.from.phi, f.to.phi, k)
      this.cam.target.lerpVectors(f.from.target, f.to.target, k)
      if (f.t >= 1) this.flight = null
    } else if (this.cinematic) {
      // Geniş çekim: yavaşça dolanan, nefes alan kamera.
      this.cineT += dt
      this.cam.theta += dt * 0.05
      this.cam.r = THREE.MathUtils.lerp(this.cam.r, 34 + Math.sin(this.cineT * 0.11) * 6, dt * 0.4)
      this.cam.phi = THREE.MathUtils.lerp(this.cam.phi, Math.PI / 2 - 0.35 + Math.sin(this.cineT * 0.07) * 0.25, dt * 0.4)
      this.cam.target.lerp(new THREE.Vector3(), dt * 0.4)
    }
    // Yeni giren meteoru kamera takip eder (kullanıcı kamerayı ellemiyorsa); bitince eski hedefe döner.
    const follow = this.vehicles.focusTarget()
    if (follow && !this.dragging && !this.flight) {
      this.beforeFollow ??= this.cam.target.clone()
      this.cam.target.lerp(follow.clone().multiplyScalar(0.6), Math.min(1, dt * 1.5))
    } else if (!follow && this.beforeFollow && !this.dragging) {
      this.flyTo({ target: this.beforeFollow }, 1.2)
      this.beforeFollow = null
    }
    this.applyCamera()
    this.camera.updateMatrixWorld()
    this.vehicles.update(dt)

    // çekirdek nabzı: etkinlikle büyür
    this.activity = Math.max(0, this.activity - dt * 0.12)
    const pulse = 1 + Math.sin(this.time * 2.2) * 0.04 + this.activity * 0.35
    this.coreGlow.scale.setScalar(6 * pulse)
    ;(this.coreGlow.material as THREE.SpriteMaterial).opacity = 0.12 + this.activity * 0.22
    this.dust.rotation.y += dt * 0.004 * slow

    // Yörüngedeki oturumlar: sırayla halkalara dağıtılır, her halkada eşit aralıklı (üst üste binmez)
    this.spin += dt * 0.07 * slow
    const orbiting = [...this.nodes.values()]
      .filter((n) => n.data.status === 'busy' || n.data.status === 'ended')
      .sort((a, b) => a.slot - b.slot)
    const perRing = RINGS.map((_, r) => orbiting.filter((_, k) => k % RINGS.length === r))
    perRing.forEach((list, r) =>
      list.forEach((n, k) => {
        n.radius = THREE.MathUtils.lerp(n.radius, RINGS[r], Math.min(1, dt * 2))
        // iç ve dış halka zıt yöne, kaydırılmış başlangıçla
        const target = (r % 2 ? -1 : 1) * this.spin + (k / list.length) * Math.PI * 2 + r * 0.6
        let d = target - n.angle
        d = Math.atan2(Math.sin(d), Math.cos(d))
        n.angle += d * Math.min(1, dt * 2.5)
      }),
    )

    const parkTmp = new THREE.Vector3()
    const home = new THREE.Vector3()
    for (const n of this.nodes.values()) {
      const parked = n.data.status === 'waiting' || n.data.status === 'idle'
      const ended = n.data.status === 'ended'
      if (parked) this.takePark(n)
      n.park = THREE.MathUtils.clamp(n.park + (parked ? dt : -dt) * 1.6, 0, 1)
      if (!parked && n.park === 0) this.freePark(n)
      n.gone = THREE.MathUtils.clamp(n.gone + (ended ? dt : -dt) * 1.5, 0, 1)

      home.set(Math.cos(n.angle) * n.radius, 0, Math.sin(n.angle) * n.radius)
      if (n.parkSlot !== null) home.lerp(this.parkPosition(n.parkSlot, parkTmp), ease(n.park))

      // sürükleme/fırlatma yayı
      if (this.dragging?.id !== n.data.id) {
        n.velocity.addScaledVector(n.offset, -dt * 28).multiplyScalar(Math.exp(-dt * 5))
        n.offset.addScaledVector(n.velocity, dt)
        if (n.offset.lengthSq() < 1e-6 && n.velocity.lengthSq() < 1e-6) n.offset.set(0, 0, 0)
      }
      n.world.copy(home).add(n.offset)
      n.group.position.copy(n.world)
      n.group.quaternion.copy(this.camera.quaternion)

      // park halinde ekranda hep aynı boyut
      const dist = this.camera.position.distanceTo(n.world)
      // Varsayılan görünümde yörüngedeki bir altıgenle aynı ekran boyutu (biraz daha büyük).
      const parkedScale = (dist / HOME.r) * 1.05
      const scale = THREE.MathUtils.lerp(1, parkedScale, ease(n.park)) * (1 - n.gone)
      n.group.scale.setScalar(Math.max(0.0001, scale))
      n.group.visible = n.gone < 1

      // seni bekleyen rozet nabız gibi parlar
      const waitingPulse = n.data.status === 'waiting' ? 0.6 + 0.4 * Math.sin(this.time * 5) : 1
      ;(n.badge.material as THREE.SpriteMaterial).opacity = waitingPulse
      ;(n.glow.material as THREE.SpriteMaterial).opacity = n.data.status === 'busy' ? 0.28 + 0.08 * Math.sin(this.time * 3) : 0.16

      // alt ajanlar: ay gibi
      n.moons.forEach((m, i) => {
        const a = this.time * 1.4 * slow + (i / n.moons.length) * Math.PI * 2
        const r = 1.05 * n.group.scale.x
        const right = new THREE.Vector3().setFromMatrixColumn(this.camera.matrixWorld, 0)
        const up = new THREE.Vector3().setFromMatrixColumn(this.camera.matrixWorld, 1)
        const fwd = new THREE.Vector3().setFromMatrixColumn(this.camera.matrixWorld, 2)
        m.position.copy(n.world).addScaledVector(right, Math.cos(a) * r).addScaledVector(up, Math.sin(a) * r * 0.45).addScaledVector(fwd, Math.sin(a) * r * 0.6)
        m.quaternion.copy(this.camera.quaternion)
        m.scale.setScalar(n.group.scale.x)
        m.visible = n.group.visible
      })

      // başlık: yörüngedeyken altıgenin altında; park halinde ya da çekirdeğin arkasında gösterilmez
      const p = n.world.clone().project(this.camera)
      const behindCore = this.occludedByCore(n.world)
      const show = n.park < 0.2 && n.gone < 0.5 && p.z < 1 && !behindCore
      n.label.style.opacity = show ? String(1 - n.park * 5) : '0'
      if (show) {
        const w = this.host.clientWidth
        const h = this.host.clientHeight
        n.label.style.transform = `translate(${((p.x + 1) / 2) * w}px, ${((1 - p.y) / 2) * h + 34}px) translateX(-50%)`
      }
    }

    // istasyonlar: ok gelince parlar; etiket istasyonun altında (çekirdeğin arkasındaysa gizli)
    const w = this.host.clientWidth
    const h = this.host.clientHeight
    for (const st of this.stations.values()) {
      st.flash = Math.max(0, st.flash - dt * 2.5)
      st.error = Math.max(0, st.error - dt * 1.5)
      const mat = st.sprite.material as THREE.SpriteMaterial
      mat.opacity = 0.75 + st.flash * 0.25
      mat.color.set(st.error > 0 ? ERROR : '#ffffff')
      st.sprite.scale.setScalar(1.45 * (1 + st.flash * 0.3))
      const p = st.pos.clone().project(this.camera)
      const show = p.z < 1 && !this.occludedByCore(st.pos)
      st.label.style.opacity = show ? '1' : '0'
      if (show) st.label.style.transform = `translate(${((p.x + 1) / 2) * w}px, ${((1 - p.y) / 2) * h + 22}px) translateX(-50%)`
      st.label.classList.toggle('hot', st.flash > 0.05)
    }

    // ışık okları
    const tmp = new THREE.Vector3()
    this.bolts = this.bolts.filter((b) => {
      b.t += dt / 0.75
      const head = Math.min(1, b.t)
      const tail = Math.max(0, head - 0.28)
      const attr = b.line.geometry.getAttribute('position') as THREE.BufferAttribute
      for (let i = 0; i < BOLT_POINTS; i++) {
        const u = tail + ((head - tail) * i) / (BOLT_POINTS - 1)
        const v = 1 - u
        tmp.set(0, 0, 0).addScaledVector(b.from, v * v).addScaledVector(b.ctrl, 2 * v * u).addScaledVector(b.to, u * u)
        attr.setXYZ(i, tmp.x, tmp.y, tmp.z)
      }
      attr.needsUpdate = true
      ;(b.line.material as THREE.LineBasicMaterial).opacity = b.t > 1 ? Math.max(0, 1 - (b.t - 1) * 4) : 1
      if (b.t >= 1 && b.t - dt / 0.75 < 1) {
        const st = this.stations.get(b.station)
        if (st) st.flash = 1
      }
      if (b.t > 1.25) {
        this.disposeObject(b.line)
        return false
      }
      return true
    })

    if (this.minimap && this.frame++ % 6 === 0) this.drawMinimap(this.minimap)
  }

  private occludedByCore(p: THREE.Vector3): boolean {
    const o = this.camera.position
    const d = p.clone().sub(o)
    const len = d.length()
    d.normalize()
    const t = -o.dot(d)
    if (t <= 0 || t >= len) return false
    const closest = o.clone().addScaledVector(d, t)
    return closest.length() < CORE_OCCLUDE
  }

  private drawMinimap(c: HTMLCanvasElement) {
    const g = c.getContext('2d')
    if (!g) return
    const w = c.width
    const h = c.height
    const s = w / 2 / 14
    g.clearRect(0, 0, w, h)
    g.fillStyle = 'rgba(3,2,8,0.75)'
    g.fillRect(0, 0, w, h)
    const cx = w / 2
    const cy = h / 2
    g.strokeStyle = 'rgba(255,176,87,0.25)'
    g.beginPath()
    g.arc(cx, cy, STATION_RADIUS * s, 0, Math.PI * 2)
    g.stroke()
    g.fillStyle = CORE
    g.beginPath()
    g.arc(cx, cy, 3, 0, Math.PI * 2)
    g.fill()
    for (const n of this.nodes.values()) {
      if (!n.group.visible) continue
      g.fillStyle = HUES[n.data.hue % HUES.length]
      g.beginPath()
      g.arc(cx + n.world.x * s, cy + n.world.z * s, n.data.status === 'busy' ? 2.5 : 3.5, 0, Math.PI * 2)
      g.fill()
    }
    for (const d of this.vehicles.dots()) {
      g.fillStyle = d.color
      g.fillRect(cx + d.x * s - 1.5, cy + d.z * s - 1.5, 3, 3)
    }
    // kamera: konum ve baktığı yön
    const camX = cx + this.camera.position.x * s
    const camZ = cy + this.camera.position.z * s
    const tx = cx + this.cam.target.x * s
    const tz = cy + this.cam.target.z * s
    const a = Math.atan2(tz - camZ, tx - camX)
    const px = cx + Math.cos(a + Math.PI) * Math.min(w, h) * 0.42
    const pz = cy + Math.sin(a + Math.PI) * Math.min(w, h) * 0.42
    g.fillStyle = '#f5ebe0'
    g.beginPath()
    g.moveTo(px + Math.cos(a) * 7, pz + Math.sin(a) * 7)
    g.lineTo(px + Math.cos(a + 2.5) * 5, pz + Math.sin(a + 2.5) * 5)
    g.lineTo(px + Math.cos(a - 2.5) * 5, pz + Math.sin(a - 2.5) * 5)
    g.fill()
  }

  private disposeObject(o: THREE.Object3D) {
    // Malzemeler atılır; paylaşılan parıltı dokusu (glowTex) sahneyle birlikte en sonda atılır.
    o.traverse((x) => {
      const m = x as THREE.Mesh
      m.geometry?.dispose()
      ;(m.material as THREE.Material | undefined)?.dispose()
    })
    o.removeFromParent()
  }

  private disposeNode(n: Node) {
    this.disposeObject(n.group)
    n.moons.forEach((m) => this.disposeObject(m))
    n.label.remove()
  }
}
