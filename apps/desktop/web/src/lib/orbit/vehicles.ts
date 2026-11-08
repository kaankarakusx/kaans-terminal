// orbit.map'in terminal tarafı (TASARIM.md §6.2–6.3):
// - Terminaller rozet (iç içe halkalar + altıgen; içinde ad ve durum). Komut çalışıyorsa terminal halkasında döner;
//   boşsa kenara park eder (aşağıdan yukarı). Halkaya katılan en geniş boşluğun ortasına girer.
// - End Session: rozet kıvılcım saçar, şok dalgası yayar, kamera kısa süre sarsılır.
// - Kapanmış terminalden kalan süreç: amber rozet. Kaçak süreç: yanan mavi kutu (meteor), kamera onu takip eder.
// (Tür adları geçmişten "car": dış API değişmedi.)
import * as THREE from 'three'
import { terminalBadge } from './hud'
import { HUES } from './palette'

export interface CarInput {
  id: string
  title: string
  busy: boolean
  /** Kapanmış terminalden kalan süreç (amber) */
  leftover?: boolean
}

export interface MeteorInput {
  pid: number
  name: string
}

export interface AppInput {
  /** pid (metin) */
  id: string
  label: string
  project: string
  cpuPct: number
}

export type Pick = { kind: 'car' | 'leftover' | 'meteor' | 'app'; id: string }

const AMBER = '#ffd23f'
const RUNNING = '#f88008'
const IDLE = '#5bd8ff'
const LANES = 1
/** Terminal halkası: oturum halkaları (4.8, 6.9) ile istasyonlar (10) arası */
const LANE_RADIUS = 8.45
/** Rozetin dünya boyutu */
const BADGE = 1.7
const PARK_DEPTH = 14
/** Uygulama arabalarının şeritleri (istasyonların dışında) */
const APP_LANES = [12.3, 13.4]
const THRUST = '#5bd8ff'
const TRAIL = 18

const ease = (t: number) => (t < 0.5 ? 4 * t * t * t : 1 - (-2 * t + 2) ** 3 / 2)

function hash(s: string): number {
  let h = 2166136261
  for (let i = 0; i < s.length; i++) h = Math.imul(h ^ s.charCodeAt(i), 16777619)
  return (h >>> 0) / 4294967295
}

interface Car {
  data: CarInput
  group: THREE.Group
  /** Rozet (tıklama da buna) */
  body: THREE.Sprite
  glow: THREE.Sprite
  key: string
  paint: THREE.Color
  lane: number
  angle: number
  /** 0 = şeritte, 1 = park etmiş */
  park: number
  parkSlot: number | null
  gone: number
  world: THREE.Vector3
}

/** Çalışan uygulama (dev sunucusu): istasyon halkasının dışında kendi şeridinde gezen araba */
interface AppCar {
  data: AppInput
  group: THREE.Group
  body: THREE.Mesh
  thrusters: THREE.Sprite[]
  trail: THREE.Line
  history: THREE.Vector3[]
  angle: number
  lane: number
  world: THREE.Vector3
  label: HTMLDivElement
  gone: number
}

interface Burst {
  points: THREE.Points
  ring: THREE.Mesh
  velocities: Float32Array
  t: number
}

interface Meteor {
  data: MeteorInput
  group: THREE.Group
  sparks: THREE.Points
  from: THREE.Vector3
  to: THREE.Vector3
  t: number
  born: number
}

export class Vehicles {
  private cars = new Map<string, Car>()
  private bursts: Burst[] = []
  private meteors = new Map<number, Meteor>()
  private apps = new Map<string, AppCar>()
  private labelLayer: HTMLDivElement | null = null
  private host: HTMLElement | null = null
  private parkSlots: (string | null)[] = []
  private jolt = 0
  private time = 0
  private tmp = new THREE.Vector3()

  constructor(
    private scene: THREE.Scene,
    private camera: THREE.PerspectiveCamera,
    private glowTex: THREE.Texture,
    private reducedMotion: boolean,
  ) {}

  // ───────────────────────── modeller ─────────────────────────

  private badgeOf(c: CarInput): { key: string; color: string; state: string } {
    const color = c.leftover ? AMBER : c.busy ? RUNNING : IDLE
    const state = c.leftover ? 'LEFTOVER' : c.busy ? 'RUNNING' : 'READY'
    return { key: `${c.title}|${state}`, color, state }
  }

  private makeCar(c: CarInput): Car {
    const b = this.badgeOf(c)
    const group = new THREE.Group()
    const glow = new THREE.Sprite(
      new THREE.SpriteMaterial({ map: this.glowTex, color: b.color, transparent: true, opacity: 0.22, blending: THREE.AdditiveBlending, depthWrite: false }),
    )
    glow.scale.setScalar(BADGE * 1.5)
    const body = new THREE.Sprite(
      new THREE.SpriteMaterial({ map: terminalBadge(c.title, b.state, b.color), transparent: true, depthWrite: false }),
    )
    body.scale.setScalar(BADGE)
    body.renderOrder = 3
    body.userData = { kind: c.leftover ? 'leftover' : 'car', id: c.id }
    group.add(glow, body)
    this.scene.add(group)

    const lane = Math.floor(hash(c.id + 'lane') * LANES)
    return {
      data: c, group, body, glow, key: b.key, paint: new THREE.Color(b.color), lane,
      angle: this.widestGap(lane),
      park: c.busy || c.leftover ? 0 : 1, parkSlot: null, gone: 0, world: new THREE.Vector3(),
    }
  }

  /** Durum ya da ad değişince rozeti yeniden çiz */
  private refresh(car: Car) {
    const b = this.badgeOf(car.data)
    if (b.key === car.key) return
    const mat = car.body.material as THREE.SpriteMaterial
    mat.map?.dispose()
    mat.map = terminalBadge(car.data.title, b.state, b.color)
    mat.needsUpdate = true
    ;(car.glow.material as THREE.SpriteMaterial).color.set(b.color)
    car.paint.set(b.color)
    car.key = b.key
  }

  /** Şeritteki en geniş boşluğun ortası (araba başka birinin üstüne binmesin). */
  private widestGap(lane: number): number {
    const angles = [...this.cars.values()]
      .filter((c) => c.lane === lane && c.park < 0.5)
      .map((c) => ((c.angle % (Math.PI * 2)) + Math.PI * 2) % (Math.PI * 2))
      .sort((a, b) => a - b)
    if (!angles.length) return Math.random() * Math.PI * 2
    let best = 0
    let at = 0
    angles.forEach((a, i) => {
      const next = i + 1 < angles.length ? angles[i + 1] : angles[0] + Math.PI * 2
      if (next - a > best) {
        best = next - a
        at = a + best / 2
      }
    })
    return at
  }

  private lanePosition(car: Car, out: THREE.Vector3) {
    // Kalan süreçler biraz dışta, aynı düzlemde
    const r = LANE_RADIUS + (car.data.leftover ? 0.9 : 0)
    return out.set(Math.cos(car.angle) * r, 0, Math.sin(car.angle) * r)
  }

  /** Arabalar alt-solda (minimap'in sağında) aşağıdan yukarı park eder; ilk aldığı yer onundur. */
  private parkPosition(slot: number, out: THREE.Vector3) {
    const perCol = 5
    const col = Math.floor(slot / perCol)
    const row = slot % perCol
    // Yön ayrı vektörde: `out` hem yön hem sonuç olursa kamera konumu yönü siler.
    const dir = this.tmp.set(-0.58 + col * 0.15, -0.82 + row * 0.2, 0.5).unproject(this.camera).sub(this.camera.position).normalize()
    return out.copy(this.camera.position).addScaledVector(dir, PARK_DEPTH)
  }

  private takePark(c: Car) {
    if (c.parkSlot !== null) return
    let i = this.parkSlots.indexOf(null)
    if (i < 0) i = this.parkSlots.push(null) - 1
    this.parkSlots[i] = c.data.id
    c.parkSlot = i
  }

  private freePark(c: Car) {
    if (c.parkSlot === null) return
    this.parkSlots[c.parkSlot] = null
    c.parkSlot = null
  }

  /** Araba etiketleri için HTML katmanı (sahnenin başlık katmanı) */
  setLabelLayer(layer: HTMLDivElement, host: HTMLElement) {
    this.labelLayer = layer
    this.host = host
  }

  private makeApp(a: AppInput): AppCar {
    const paint = HUES[Math.floor(hash(a.project) * HUES.length)]
    const mat = () => new THREE.MeshStandardMaterial({ color: paint, metalness: 0.75, roughness: 0.22 })
    const group = new THREE.Group()
    const body = new THREE.Mesh(new THREE.BoxGeometry(1.0, 0.24, 0.5), mat())
    body.userData = { kind: 'app', id: a.id }
    const nose = new THREE.Mesh(new THREE.BoxGeometry(0.28, 0.16, 0.44), mat())
    nose.position.set(0.6, -0.03, 0)
    const cabin = new THREE.Mesh(
      new THREE.BoxGeometry(0.46, 0.17, 0.4),
      new THREE.MeshStandardMaterial({ color: '#0b1a2a', metalness: 1, roughness: 0.05, transparent: true, opacity: 0.85 }),
    )
    cabin.position.set(0.05, 0.2, 0)
    const thrusters = [-0.15, 0.15].map((z) => {
      const t = new THREE.Sprite(
        new THREE.SpriteMaterial({ map: this.glowTex, color: THRUST, transparent: true, blending: THREE.AdditiveBlending, depthWrite: false }),
      )
      t.position.set(-0.6, 0, z)
      t.scale.setScalar(0.45)
      return t
    })
    group.add(body, nose, cabin, ...thrusters)
    this.scene.add(group)
    const trail = new THREE.Line(
      new THREE.BufferGeometry().setAttribute('position', new THREE.BufferAttribute(new Float32Array(TRAIL * 3), 3)),
      new THREE.LineBasicMaterial({ color: THRUST, transparent: true, opacity: 0.35, blending: THREE.AdditiveBlending, depthWrite: false }),
    )
    trail.frustumCulled = false
    this.scene.add(trail)
    const label = document.createElement('div')
    label.className = 'app-label'
    label.innerHTML = '<b></b><span></span>'
    this.labelLayer?.appendChild(label)
    const lane = this.apps.size % APP_LANES.length
    return { data: a, group, body, thrusters, trail, history: [], angle: hash(a.id) * Math.PI * 2, lane, world: new THREE.Vector3(), label, gone: 0 }
  }

  /** Çalışan uygulamalar (dev sunucuları): arabalar */
  setApps(list: AppInput[]) {
    const ids = new Set(list.map((a) => a.id))
    for (const a of list) {
      let car = this.apps.get(a.id)
      if (!car) {
        car = this.makeApp(a)
        this.apps.set(a.id, car)
      }
      car.data = a
      ;(car.label.firstChild as HTMLElement).textContent = a.label
      ;(car.label.lastChild as HTMLElement).textContent = `${a.project} · ${Math.round(a.cpuPct)}% cpu`
    }
    for (const [id, car] of this.apps) if (!ids.has(id) && car.gone === 0) car.gone = 0.0001
  }

  private removeApp(car: AppCar) {
    this.disposeObject(car.group)
    this.disposeObject(car.trail)
    car.label.remove()
    this.apps.delete(car.data.id)
  }

  // ───────────────────────── dış API ─────────────────────────

  setCars(list: CarInput[]) {
    const ids = new Set(list.map((c) => c.id))
    for (const c of list) {
      const car = this.cars.get(c.id)
      if (!car) {
        this.cars.set(c.id, this.makeCar(c))
        continue
      }
      // Şeride yeni katılıyorsa en geniş boşluğa girer.
      if (c.busy && !car.data.busy && car.park > 0.5) car.angle = this.widestGap(car.lane)
      car.data = c
      this.refresh(car)
    }
    for (const [id, car] of this.cars) {
      if (!ids.has(id) && car.gone === 0) car.gone = 0.0001 // sessizce küçülüp kaybolur
    }
  }

  /** End Session: araba patlar. */
  explode(id: string) {
    const car = this.cars.get(id)
    if (!car) return
    const pos = car.world.clone()
    const paint = car.paint.clone()
    this.remove(car)
    const n = 90
    const positions = new Float32Array(n * 3)
    const colors = new Float32Array(n * 3)
    const velocities = new Float32Array(n * 3)
    const hot = new THREE.Color('#ffb057')
    for (let i = 0; i < n; i++) {
      positions.set([pos.x, pos.y, pos.z], i * 3)
      const v = new THREE.Vector3().randomDirection().multiplyScalar(2 + Math.random() * 5)
      velocities.set([v.x, v.y, v.z], i * 3)
      const c = i % 3 === 0 ? hot : paint
      colors.set([c.r, c.g, c.b], i * 3)
    }
    const geo = new THREE.BufferGeometry()
    geo.setAttribute('position', new THREE.BufferAttribute(positions, 3))
    geo.setAttribute('color', new THREE.BufferAttribute(colors, 3))
    const points = new THREE.Points(
      geo,
      new THREE.PointsMaterial({ size: 0.28, map: this.glowTex, vertexColors: true, transparent: true, blending: THREE.AdditiveBlending, depthWrite: false }),
    )
    const ring = new THREE.Mesh(
      new THREE.RingGeometry(0.4, 0.55, 48),
      new THREE.MeshBasicMaterial({ color: '#ffd9a8', transparent: true, blending: THREE.AdditiveBlending, depthWrite: false, side: THREE.DoubleSide }),
    )
    ring.position.copy(pos)
    this.scene.add(points, ring)
    this.bursts.push({ points, ring, velocities, t: 0 })
    if (!this.reducedMotion) this.jolt = 1
  }

  setMeteors(list: MeteorInput[]) {
    const pids = new Set(list.map((m) => m.pid))
    for (const m of list) {
      if (this.meteors.has(m.pid)) continue
      const group = new THREE.Group()
      const box = new THREE.Mesh(
        new THREE.BoxGeometry(0.7, 0.7, 0.7),
        new THREE.MeshStandardMaterial({ color: '#1d4fff', emissive: '#3aa0ff', emissiveIntensity: 2.2, metalness: 0.2, roughness: 0.4 }),
      )
      box.userData = { kind: 'meteor', id: String(m.pid) }
      const glow = new THREE.Sprite(
        new THREE.SpriteMaterial({ map: this.glowTex, color: '#3aa0ff', transparent: true, blending: THREE.AdditiveBlending, depthWrite: false }),
      )
      glow.scale.setScalar(3)
      group.add(glow, box)
      const sparks = new THREE.Points(
        new THREE.BufferGeometry().setAttribute('position', new THREE.BufferAttribute(new Float32Array(40 * 3), 3)),
        new THREE.PointsMaterial({ size: 0.2, map: this.glowTex, color: '#7fc4ff', transparent: true, blending: THREE.AdditiveBlending, depthWrite: false }),
      )
      sparks.frustumCulled = false
      this.scene.add(group, sparks)
      const to = new THREE.Vector3(4.5 + Math.random() * 2, 2.5 + Math.random() * 1.5, 2)
      const from = to.clone().add(new THREE.Vector3(30, 22, -40))
      this.meteors.set(m.pid, { data: m, group, sparks, from, to, t: 0, born: this.time })
    }
    for (const [pid, m] of this.meteors) {
      if (!pids.has(pid)) {
        this.disposeObject(m.group)
        this.disposeObject(m.sparks)
        this.meteors.delete(pid)
      }
    }
  }

  /** Raycast hedefleri (araba gövdesi, meteor kutusu). */
  pickables(): THREE.Object3D[] {
    return [
      ...[...this.cars.values()].filter((c) => c.gone === 0).map((c) => c.body),
      ...[...this.apps.values()].filter((c) => c.gone === 0).map((c) => c.body),
      ...[...this.meteors.values()].map((m) => m.group.children[1]),
    ]
  }

  /** Yeni giren meteorun konumu: kamera ilk birkaç saniye onu takip eder. */
  focusTarget(): THREE.Vector3 | null {
    for (const m of this.meteors.values()) if (this.time - m.born < 6) return m.group.position
    return null
  }

  /** Kamera sarsıntısı ofseti (patlamada). */
  joltOffset(out: THREE.Vector3): THREE.Vector3 {
    if (this.jolt <= 0) return out.set(0, 0, 0)
    const a = this.jolt * 0.35
    return out.set((Math.random() - 0.5) * a, (Math.random() - 0.5) * a, (Math.random() - 0.5) * a)
  }

  dots(): { x: number; z: number; color: string }[] {
    return [
      ...[...this.cars.values()].filter((c) => c.gone === 0).map((c) => ({
        x: c.world.x, z: c.world.z, color: c.data.leftover ? AMBER : c.data.busy ? RUNNING : IDLE,
      })),
      ...[...this.meteors.values()].map((m) => ({ x: m.group.position.x, z: m.group.position.z, color: '#3aa0ff' })),
      ...[...this.apps.values()].filter((c) => c.gone === 0).map((c) => ({ x: c.world.x, z: c.world.z, color: THRUST })),
    ]
  }

  // ───────────────────────── döngü ─────────────────────────

  update(dt: number) {
    this.time += dt
    this.jolt = Math.max(0, this.jolt - dt * 2.2)
    const slow = this.reducedMotion ? 0.3 : 1
    const lanePos = new THREE.Vector3()
    const parkPos = new THREE.Vector3()

    for (const car of [...this.cars.values()]) {
      if (car.gone > 0) {
        car.gone += dt * 2.5
        if (car.gone >= 1) {
          this.remove(car)
          continue
        }
      }
      const cruising = car.data.busy || !!car.data.leftover
      if (!cruising) this.takePark(car)
      car.park = THREE.MathUtils.clamp(car.park + (cruising ? -dt : dt) * 1.4, 0, 1)
      if (cruising && car.park === 0) this.freePark(car)
      if (cruising) car.angle += dt * (car.data.leftover ? 0.12 : 0.26) * slow

      this.lanePosition(car, lanePos)
      const k = ease(car.park)
      if (car.parkSlot !== null) car.world.lerpVectors(lanePos, this.parkPosition(car.parkSlot, parkPos), k)
      else car.world.copy(lanePos)
      car.group.position.copy(car.world)
      car.group.quaternion.copy(this.camera.quaternion)
      // Park halinde ekranda hep aynı boyut
      const dist = this.camera.position.distanceTo(car.world)
      const scale = THREE.MathUtils.lerp(1, (dist / 27) * 0.9, k) * (1 - car.gone)
      car.group.scale.setScalar(Math.max(0.0001, scale))
      // Çalışırken hafifçe nefes alır
      ;(car.glow.material as THREE.SpriteMaterial).opacity = cruising ? 0.22 + 0.1 * Math.sin(this.time * 3 + car.angle) : 0.12
    }

    // uygulama arabaları: CPU arttıkça hızlanır, iticiler parlar; arkada kısa iz
    const w = this.host?.clientWidth ?? 0
    const h = this.host?.clientHeight ?? 0
    for (const car of [...this.apps.values()]) {
      if (car.gone > 0) {
        car.gone += dt * 2.5
        if (car.gone >= 1) {
          this.removeApp(car)
          continue
        }
      }
      const load = Math.min(1, car.data.cpuPct / 100)
      car.angle += dt * (0.09 + load * 0.35) * slow
      const r = APP_LANES[car.lane]
      car.world.set(Math.cos(car.angle) * r, 0.5 + Math.sin(this.time * 1.3 + car.angle * 3) * 0.12, Math.sin(car.angle) * r)
      car.group.position.copy(car.world)
      const tangent = new THREE.Vector3(-Math.sin(car.angle), 0, Math.cos(car.angle))
      car.group.rotation.set(0, Math.atan2(-tangent.z, tangent.x), -0.08 + Math.sin(this.time * 1.7 + car.angle) * 0.03)
      car.group.scale.setScalar(Math.max(0.0001, 0.95 * (1 - car.gone)))
      for (const t of car.thrusters) t.scale.setScalar(0.25 + load * 0.4 + Math.random() * 0.1)
      car.history.unshift(car.world.clone())
      car.history.length = Math.min(car.history.length, TRAIL)
      const attr = car.trail.geometry.getAttribute('position') as THREE.BufferAttribute
      for (let i = 0; i < TRAIL; i++) {
        const p = car.history[Math.min(i, car.history.length - 1)] ?? car.world
        attr.setXYZ(i, p.x, p.y, p.z)
      }
      attr.needsUpdate = true
      car.trail.visible = car.history.length > 2
      // etiket: arabanın altında
      const p = car.world.clone().project(this.camera)
      const show = p.z < 1 && car.gone === 0 && w > 0
      car.label.style.opacity = show ? '1' : '0'
      if (show) car.label.style.transform = `translate(${((p.x + 1) / 2) * w}px, ${((1 - p.y) / 2) * h + 16}px) translateX(-50%)`
    }

    // patlamalar
    this.bursts = this.bursts.filter((b) => {
      b.t += dt
      const attr = b.points.geometry.getAttribute('position') as THREE.BufferAttribute
      for (let i = 0; i < attr.count; i++) {
        const drag = Math.exp(-b.t * 2)
        attr.setXYZ(
          i,
          attr.getX(i) + b.velocities[i * 3] * dt * drag,
          attr.getY(i) + b.velocities[i * 3 + 1] * dt * drag,
          attr.getZ(i) + b.velocities[i * 3 + 2] * dt * drag,
        )
      }
      attr.needsUpdate = true
      ;(b.points.material as THREE.PointsMaterial).opacity = Math.max(0, 1 - b.t / 1.2)
      b.ring.scale.setScalar(1 + b.t * 14)
      b.ring.quaternion.copy(this.camera.quaternion)
      ;(b.ring.material as THREE.MeshBasicMaterial).opacity = Math.max(0, 1 - b.t / 0.7)
      if (b.t > 1.3) {
        this.disposeObject(b.points)
        this.disposeObject(b.ring)
        return false
      }
      return true
    })

    // meteorlar: uzaktan girer, sonra yanarak döner
    for (const m of this.meteors.values()) {
      m.t = Math.min(1, m.t + dt / 3)
      const p = m.group.position.lerpVectors(m.from, m.to, ease(m.t))
      if (m.t >= 1) p.y += Math.sin(this.time * 1.3) * 0.2
      m.group.rotation.x += dt * 0.9
      m.group.rotation.y += dt * 1.3
      const sparks = m.sparks.geometry.getAttribute('position') as THREE.BufferAttribute
      for (let i = 0; i < sparks.count; i++) {
        const back = (i / sparks.count) * (m.t < 1 ? 6 : 1.2)
        const dir = m.from.clone().sub(m.to).normalize()
        sparks.setXYZ(
          i,
          p.x + dir.x * back + (Math.random() - 0.5) * 0.5,
          p.y + dir.y * back + (Math.random() - 0.5) * 0.5,
          p.z + dir.z * back + (Math.random() - 0.5) * 0.5,
        )
      }
      sparks.needsUpdate = true
    }
  }

  private remove(car: Car) {
    this.freePark(car)
    this.disposeObject(car.group)
    this.cars.delete(car.data.id)
  }

  private disposeObject(o: THREE.Object3D) {
    o.traverse((x) => {
      const m = x as THREE.Mesh
      m.geometry?.dispose()
      ;(m.material as THREE.Material | undefined)?.dispose()
    })
    o.removeFromParent()
  }

  dispose() {
    for (const c of [...this.cars.values()]) this.remove(c)
    for (const a of [...this.apps.values()]) this.removeApp(a)
    for (const b of this.bursts) {
      this.disposeObject(b.points)
      this.disposeObject(b.ring)
    }
    for (const m of this.meteors.values()) {
      this.disposeObject(m.group)
      this.disposeObject(m.sparks)
    }
  }
}
