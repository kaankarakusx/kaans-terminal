// Kayıtlı sunucular (Rust: src-tauri/src/servers.rs). Bağlan = `ssh` çalıştıran yeni terminal penceresi.
// Tauri dışında (tarayıcı demosu) bellekte sahte liste.
import { invoke } from '@tauri-apps/api/core'
import { inTauri } from './engine.svelte'
import { openTerminal } from './terminals'
import * as api from '../term/api'

export interface Server {
  id: string
  name: string
  host: string
  user: string | null
  port: number | null
  identityFile: string | null
  group: string | null
  lastUsed: number | null
}

export interface SshHost {
  alias: string
  hostName: string | null
  user: string | null
  port: number | null
  proxy: boolean
}

export type Reach = { state: 'up' | 'down' | 'unknown' | 'checking'; ms: number | null }

const demo: { servers: Server[]; ssh: SshHost[] } = {
  servers: [
    { id: 'srv-1', name: 'prod api', host: 'api.example.com', user: 'deploy', port: null, identityFile: null, group: 'prod', lastUsed: Date.now() - 3_600_000 },
    { id: 'srv-2', name: 'prod db', host: 'db.example.com', user: 'root', port: null, identityFile: null, group: 'prod', lastUsed: Date.now() - 86_400_000 * 3 },
    { id: 'srv-3', name: 'staging', host: 'staging.example.com', user: 'ubuntu', port: 2222, identityFile: '~/.ssh/id_ed25519', group: 'staging', lastUsed: null },
    { id: 'srv-4', name: 'raspberry', host: 'pi.local', user: 'pi', port: null, identityFile: null, group: null, lastUsed: Date.now() - 600_000 },
  ],
  ssh: [
    { alias: 'homelab', hostName: '192.168.1.20', user: 'pi', port: null, proxy: false },
    { alias: 'gh-runner', hostName: 'runner.internal', user: null, port: 2201, proxy: true },
  ],
}

class Servers {
  list = $state<Server[]>([])
  sshConfig = $state<SshHost[]>([])
  loaded = $state(false)
  error = $state<string | null>(null)
  /** Bağlanmakta olan sunucu (çift tık iki pencere açmasın) */
  connecting = $state<string | null>(null)
  /** Ulaşılabilirlik (sunucu kimliği → durum) */
  reach = $state<Record<string, Reach>>({})
  /** Açık ssh oturumları (sunucu kimliği → terminal oturumları) */
  open = $state<Record<string, string[]>>({})

  async refresh() {
    try {
      if (!inTauri) {
        this.list = [...demo.servers]
        this.sshConfig = demo.ssh
      } else {
        const s = await invoke<{ servers: Server[]; sshConfig: SshHost[] }>('servers_list')
        this.list = s.servers
        this.sshConfig = s.sshConfig
      }
      this.loaded = true
    } catch (e) {
      this.error = String(e)
    }
  }

  /** TCP ile ssh portunu yoklar (yalnızca sayfa açıkken çağrılır). */
  async probe() {
    for (const s of this.list) this.reach[s.id] = { state: 'checking', ms: this.reach[s.id]?.ms ?? null }
    try {
      const res: { id: string; state: Reach['state']; ms: number | null }[] = inTauri
        ? await invoke('servers_probe')
        : await new Promise((r) =>
            setTimeout(() => r(this.list.map((s, i) => ({ id: s.id, state: i === 1 ? 'down' : 'up', ms: i === 1 ? null : 18 + i * 37 }))), 600),
          )
      const next: Record<string, Reach> = {}
      for (const p of res) next[p.id] = { state: p.state, ms: p.ms }
      this.reach = next
    } catch {
      this.reach = {}
    }
  }

  /** Terminal oturumlarından hangi sunucuya kaç bağlantı açık */
  track(sessions: api.SessionInfo[]) {
    const next: Record<string, string[]> = {}
    for (const t of sessions) {
      if (t.command[0] !== 'ssh') continue
      const dest = t.command[t.command.length - 1]
      const s = this.list.find((x) => destOf(x) === dest)
      if (s) next[s.id] = [...(next[s.id] ?? []), t.id]
    }
    this.open = next
  }

  /** Kaydeder; hata metnini döner (yoksa null). Kaydedilen sunucu `saved`'da. */
  saved = $state<Server | null>(null)
  async save(server: Server): Promise<string | null> {
    try {
      if (!inTauri) {
        const s = { ...server, id: server.id || `srv-${Date.now()}`, name: server.name || server.host }
        demo.servers = [...demo.servers.filter((x) => x.id !== s.id), s]
        this.list = [...demo.servers]
        this.saved = s
      } else {
        const before = new Set(this.list.map((x) => x.id))
        this.list = await invoke<Server[]>('servers_save', { server })
        this.saved = this.list.find((x) => (server.id ? x.id === server.id : !before.has(x.id))) ?? null
      }
      return null
    } catch (e) {
      return String(e)
    }
  }

  async remove(id: string) {
    if (!inTauri) {
      demo.servers = demo.servers.filter((x) => x.id !== id)
      this.list = [...demo.servers]
      return
    }
    this.list = await invoke<Server[]>('servers_delete', { id })
  }

  async connect(s: Server) {
    if (this.connecting) return
    this.connecting = s.id
    this.error = null
    try {
      const session = inTauri
        ? await invoke<string>('server_connect', { id: s.id, cols: 100, rows: 30 })
        : await api.createTerminal(null, 100, 30, sshCommand(s))
      await api.openWindow({ session, title: s.name })
      if (inTauri) void this.refresh()
      else {
        demo.servers = demo.servers.map((x) => (x.id === s.id ? { ...x, lastUsed: Date.now() } : x))
        this.list = [...demo.servers]
      }
    } catch (e) {
      this.error = String(e)
    } finally {
      this.connecting = null
    }
  }

  /** Açık bağlantının penceresini öne getirir */
  async focus(s: Server) {
    const id = this.open[s.id]?.[0]
    if (id) await openTerminal(id, s.name)
  }
}

export const servers = new Servers()

/** ssh hedefi: `kullanıcı@host` ya da `host` (Rust `ssh_argv` ile aynı) */
export function destOf(s: { host: string; user: string | null }): string {
  return s.user ? `${s.user}@${s.host}` : s.host
}

/** Rust'ın çalıştıracağı komutun eşi (önizleme ve kopyalama için; `~` açılmaz) */
export function sshCommand(s: Pick<Server, 'host' | 'user' | 'port' | 'identityFile'>): string[] {
  return ['ssh', ...(s.port ? ['-p', String(s.port)] : []), ...(s.identityFile ? ['-i', s.identityFile] : []), destOf(s)]
}

/** Kabukta yapıştırılabilir metin */
export function shellLine(argv: string[]): string {
  return argv.map((a) => (/^[\w@%+=:,./~-]+$/.test(a) ? a : `'${a.replace(/'/g, `'\\''`)}'`)).join(' ')
}

/** `kullanıcı@host:port` gösterimi */
export function target(s: { host: string; user: string | null; port?: number | null }): string {
  return `${destOf(s)}${s.port ? `:${s.port}` : ''}`
}

export function blank(): Server {
  return { id: '', name: '', host: '', user: null, port: null, identityFile: null, group: null, lastUsed: null }
}

/** Grup rengi: adına göre sabit bir `--hueN` token'ı */
export function groupHue(g: string): string {
  let h = 0
  for (const c of g) h = (h * 31 + c.charCodeAt(0)) >>> 0
  return `var(--hue${h % 10})`
}
