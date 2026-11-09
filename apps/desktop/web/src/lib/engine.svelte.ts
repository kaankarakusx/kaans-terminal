// Engine ile köprü: Rust'tan gelen anlık görüntüleri, olayları ve güç durumunu tutar.
// Tauri dışında (tarayıcıda `npm run dev`) demo verisiyle çalışır; demo kodu üretim paketine girmez.
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { EngineEvent, PowerState, SessionView, Snapshot } from './types'

// src-tauri/src/lib.rs ile aynı adlar
const EV_STATE = 'engine://state'
const EV_EVENTS = 'engine://events'
const EV_POWER = 'power://state'
const EV_PROFILE = 'profile://changed'
const RING = 200

export const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

class EngineStore {
  snapshot = $state.raw<Snapshot | null>(null)
  /** Son olaylar, eskiden yeniye. */
  events = $state.raw<EngineEvent[]>([])
  /** Hata dönen araç çağrılarının id'leri. */
  failed = $state.raw<ReadonlySet<string>>(new Set())
  power = $state.raw<PowerState>({ onBattery: false, lowPower: false })
  connected = $state(false)
  demo = $state(false)

  sessionsById = $derived(new Map<string, SessionView>((this.snapshot?.sessions ?? []).map((s) => [s.id, s])))

  /** Yeni olay dinleyicileri (orbit.map okları, sesler). */
  private listeners = new Set<(batch: EngineEvent[]) => void>()

  onEvents(fn: (batch: EngineEvent[]) => void): () => void {
    this.listeners.add(fn)
    return () => this.listeners.delete(fn)
  }

  async start() {
    if (!inTauri) {
      if (import.meta.env.DEV) {
        const { startDemo } = await import('./demo')
        this.demo = true
        this.connected = true
        startDemo((snap) => (this.snapshot = snap), (batch) => this.push(batch))
      }
      return
    }
    await listen<Snapshot>(EV_STATE, (e) => {
      this.snapshot = e.payload
    })
    await listen<EngineEvent[]>(EV_EVENTS, (e) => this.push(e.payload))
    await listen<PowerState>(EV_POWER, (e) => {
      this.power = e.payload
    })
    // Claude profili değişti: önceki profilin olayları (session.log, orbit okları) gitsin; engine yeniden kurulur.
    await listen(EV_PROFILE, () => {
      this.events = []
      this.failed = new Set()
    })
    const init = await invoke<{ snapshot: Snapshot | null; events: EngineEvent[]; power: PowerState }>(
      'engine_initial',
    )
    if (!this.snapshot && init.snapshot) this.snapshot = init.snapshot
    this.power = init.power
    if (this.events.length === 0) this.push(init.events, false)
    this.connected = true
  }

  private push(batch: EngineEvent[], notify = true) {
    if (batch.length === 0) return
    const next = [...this.events, ...batch].slice(-RING)
    const failed = new Set(this.failed)
    for (const e of batch) if (e.kind === 'toolError' && e.id) failed.add(e.id)
    this.events = next
    this.failed = failed
    if (notify) for (const fn of this.listeners) fn(batch)
  }
}

export const engine = new EngineStore()
