// Claude profilleri (Rust `profiles.rs`): ayrı CLAUDE_CONFIG_DIR klasörleri (ör. `claude-work` → ~/.claude-work).
// Seçili profil engine'in okuduğu klasördür: sohbetler, maliyet, limitler, harcama defteri ona göre.
import { invoke } from '@tauri-apps/api/core'
import { inTauri } from './engine.svelte'

export interface ClaudeProfile {
  id: string
  name: string
  dir: string
  commands: string[]
  active: boolean
}

const DEMO: ClaudeProfile[] = [
  { id: '', name: 'personal', dir: '/Users/demo/.claude', commands: ['claude', 'claude-personal'], active: true },
  { id: 'claude-work', name: 'work', dir: '/Users/demo/.claude-work', commands: ['claude-work'], active: false },
]

/** Ev klasörü `~` olarak */
export const tilde = (p: string) => p.replace(/^\/Users\/[^/]+|^\/home\/[^/]+|^[A-Za-z]:\\Users\\[^\\]+/, '~')

class Profiles {
  list = $state.raw<ClaudeProfile[]>([])
  busy = $state(false)
  error = $state<string | null>(null)

  get active(): ClaudeProfile | undefined {
    return this.list.find((p) => p.active)
  }

  /** Seçili profilin klasörü (`~/.claude-work`); bilinmiyorsa `~/.claude` */
  get dir(): string {
    return tilde(this.active?.dir ?? '~/.claude')
  }

  async load() {
    try {
      this.list = inTauri ? await invoke<ClaudeProfile[]>('profiles_list') : structuredClone(DEMO)
    } catch (e) {
      this.error = String(e)
    }
  }

  async select(p: ClaudeProfile) {
    if (p.active || this.busy) return
    this.busy = true
    this.error = null
    try {
      if (inTauri) await invoke('profile_set', { dir: p.dir })
      else for (const d of DEMO) d.active = d.dir === p.dir
      await this.load()
    } catch (e) {
      this.error = String(e)
    } finally {
      this.busy = false
    }
  }
}

export const profiles = new Profiles()
