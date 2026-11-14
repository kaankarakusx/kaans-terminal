// Entegrasyonlar (Rust: src-tauri/src/integrations.rs). Harici ayarlar yalnızca kullanıcı açınca değişir.
import { invoke } from '@tauri-apps/api/core'
import { inTauri } from './engine.svelte'

export interface IntegrationStatus {
  id: string
  enabled: boolean
  chained: string | null
  error: string | null
  devBuild: boolean
  /** statusLine uygulamanın başka bir kopyasını çağırıyor (o kopyanın komutu) */
  otherCopy?: string | null
}

export interface SaverStatus {
  id: string
  enabled: boolean
  agents: string[]
  /** "yours" | "bundled" | "missing" | "plugin" */
  source: string
  detail: string | null
  error: string | null
}

const demo: IntegrationStatus[] = [
  { id: 'claude-statusline', enabled: false, chained: '~/bin/my-status', error: null, devBuild: true },
  { id: 'claude-theme', enabled: false, chained: 'dark', error: null, devBuild: false },
]
const demoSavers: SaverStatus[] = [
  { id: 'rtk', enabled: false, agents: [], source: 'missing', detail: 'rtk-oss v1.1.0 · Apache-2.0', error: null },
  { id: 'caveman', enabled: false, agents: [], source: 'plugin', detail: 'Claude plugin caveman@caveman', error: null },
]

export async function savers(): Promise<SaverStatus[]> {
  if (!inTauri) return demoSavers
  return invoke<SaverStatus[]>('savers_status')
}

export async function status(): Promise<IntegrationStatus[]> {
  if (!inTauri) return demo
  return invoke<IntegrationStatus[]>('integrations_status')
}

export async function setEnabled(id: string, enabled: boolean): Promise<void> {
  if (!inTauri) {
    const d = demo.find((x) => x.id === id) ?? demoSavers.find((x) => x.id === id)
    if (d) d.enabled = enabled
    return
  }
  return invoke('integration_set', { id, enabled })
}

export async function removeAll(): Promise<void> {
  if (!inTauri) {
    demo.forEach((d) => (d.enabled = false))
    demoSavers.forEach((d) => (d.enabled = false))
    return
  }
  return invoke('integrations_remove_all')
}
