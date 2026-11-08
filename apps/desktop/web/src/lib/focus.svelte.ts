// Bir oturuma dikkat çekme: kart görünür alana kayar ve kısa süre parlar (bildirim, harita, your.call tıklaması).
// Sohbet penceresini açmak terminal katmanıyla (Aşama 3) gelir.
import { layout } from './layout.svelte'

class Focus {
  id = $state<string | null>(null)
  private timer: ReturnType<typeof setTimeout> | undefined

  session(id: string) {
    layout.show('sessions')
    this.id = id
    clearTimeout(this.timer)
    this.timer = setTimeout(() => (this.id = null), 2200)
  }
}

export const focus = new Focus()
