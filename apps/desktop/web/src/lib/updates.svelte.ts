// Otomatik güncelleme durumu (Rust `updates.rs`): `update://state` olayı + komutlar. Tarayıcı demosunda sahte durum.
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { inTauri } from './engine.svelte'

export interface UpdateView {
  status: 'disabled' | 'idle' | 'checking' | 'downloading' | 'ready' | 'installing' | 'error'
  current: string
  version: string | null
  notes: string | null
  pct: number | null
  error: string | null
  checked: number | null
}

export interface WhatsNew {
  version: string
  notes: string
}

class Updates {
  view = $state<UpdateView>({
    status: 'disabled',
    current: '0.1.0',
    version: null,
    notes: null,
    pct: null,
    error: null,
    checked: null,
  })
  /** Bu sürümün ilk açılışında gösterilecek notlar */
  whatsNew = $state<WhatsNew | null>(null)

  async start() {
    if (!inTauri) return
    this.view = await invoke<UpdateView>('update_state')
    await listen<UpdateView>('update://state', (e) => (this.view = e.payload))
    this.whatsNew = await invoke<WhatsNew | null>('update_whats_new')
  }

  async check() {
    if (!inTauri) return
    await invoke('update_check')
  }

  async install() {
    if (!inTauri) return
    try {
      await invoke('update_install')
    } catch (e) {
      this.view = { ...this.view, status: 'error', error: String(e) }
    }
  }
}

export const updates = new Updates()
