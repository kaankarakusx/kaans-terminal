// Ön plan / arka plan, pil ve kullanıcı hareketsizliği (OZELLIKLER.md §2.3, TASARIM.md §6.4).
// Uygulama arkadayken <html class="behind"> olur ve CSS animasyonları durur.
import { engine } from './engine.svelte'

export type PowerMode = 'foreground' | 'background' | 'hidden'

class Power {
  focused = $state(typeof document === 'undefined' ? true : document.hasFocus())
  visible = $state(typeof document === 'undefined' ? true : !document.hidden)
  /** Son klavye/fare hareketi (sinematik kamera 5 dk hareketsizlikte başlar). */
  lastInput = $state(Date.now())

  mode: PowerMode = $derived(!this.visible ? 'hidden' : this.focused ? 'foreground' : 'background')
  /** Pilde ya da düşük güç modunda: kare hızı tavanı, sinematik kamera kendiliğinden başlamaz. */
  saver = $derived(engine.power.onBattery || engine.power.lowPower)

  constructor() {
    if (typeof window === 'undefined') return
    const sync = () => {
      this.focused = document.hasFocus()
      this.visible = !document.hidden
      document.documentElement.classList.toggle('behind', !(this.focused && this.visible))
    }
    window.addEventListener('focus', sync)
    window.addEventListener('blur', sync)
    document.addEventListener('visibilitychange', sync)
    sync()
    let last = 0
    const touch = () => {
      const now = Date.now()
      // saniyede birden sık yazma (her fare hareketinde reaktif güncelleme olmasın)
      if (now - last > 1000) {
        last = now
        this.lastInput = now
      }
    }
    for (const ev of ['pointermove', 'pointerdown', 'keydown', 'wheel'] as const) {
      window.addEventListener(ev, touch, { passive: true })
    }
  }
}

export const power = new Power()
