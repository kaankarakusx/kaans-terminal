// Kısayollar (OZELLIKLER.md §3.3–3.6): tek kayıt defteri; kullanıcı SHORTCUTS ekranından değiştirebilir.
// Tuşlar fiziksel koduyla (`e.code`) eşlenir: ⌥ basılıyken `[` başka karakter üretse de, Türkçe klavyede de çalışır.
// "Mod" = Mac'te ⌘, Windows'ta Ctrl.

export const isMac = typeof navigator !== 'undefined' && /mac/i.test(navigator.platform)

export type Scope = 'term' | 'app'

export interface ShortcutDef {
  id: string
  label: string
  scope: Scope
  combo: string
}

const DEFAULTS: ShortcutDef[] = [
  { id: 'term.newTab', label: 'new tab', scope: 'term', combo: 'Mod+KeyT' },
  { id: 'term.splitRight', label: 'split right', scope: 'term', combo: 'Mod+KeyD' },
  { id: 'term.splitDown', label: 'split down', scope: 'term', combo: 'Mod+Shift+KeyD' },
  { id: 'term.nextPane', label: 'next pane', scope: 'term', combo: 'Mod+BracketRight' },
  { id: 'term.prevPane', label: 'previous pane', scope: 'term', combo: 'Mod+BracketLeft' },
  { id: 'term.closePane', label: 'close pane (keeps running)', scope: 'term', combo: 'Mod+KeyW' },
  { id: 'term.reopenTab', label: 'reopen closed tab', scope: 'term', combo: 'Mod+Shift+KeyT' },
  { id: 'term.moveTabLeft', label: 'move tab left', scope: 'term', combo: 'Mod+Alt+Shift+BracketLeft' },
  { id: 'term.moveTabRight', label: 'move tab right', scope: 'term', combo: 'Mod+Alt+Shift+BracketRight' },
  { id: 'term.growLeft', label: 'resize pane ←', scope: 'term', combo: 'Mod+Ctrl+ArrowLeft' },
  { id: 'term.growRight', label: 'resize pane →', scope: 'term', combo: 'Mod+Ctrl+ArrowRight' },
  { id: 'term.growUp', label: 'resize pane ↑', scope: 'term', combo: 'Mod+Ctrl+ArrowUp' },
  { id: 'term.growDown', label: 'resize pane ↓', scope: 'term', combo: 'Mod+Ctrl+ArrowDown' },
  { id: 'term.find', label: 'find', scope: 'term', combo: 'Mod+KeyF' },
  { id: 'term.findNext', label: 'find next', scope: 'term', combo: 'Mod+KeyG' },
  { id: 'term.findPrev', label: 'find previous', scope: 'term', combo: 'Mod+Shift+KeyG' },
  { id: 'term.useSelection', label: 'use selection for find', scope: 'term', combo: 'Mod+KeyE' },
  { id: 'term.clear', label: 'clear buffer', scope: 'term', combo: 'Mod+KeyK' },
  { id: 'term.copyLast', label: 'copy last answer', scope: 'term', combo: 'Mod+Shift+KeyC' },
  { id: 'term.fullscreen', label: 'full screen', scope: 'term', combo: 'Mod+Enter' },
  { id: 'term.endSession', label: 'end session', scope: 'term', combo: 'Mod+Shift+KeyE' },
  { id: 'app.newTerminal', label: '+ new terminal', scope: 'app', combo: 'Mod+KeyN' },
  { id: 'app.toggleSessions', label: 'toggle chat list', scope: 'app', combo: 'Mod+KeyB' },
  { id: 'app.wheel', label: 'shortcut wheel', scope: 'app', combo: isMac ? 'Ctrl+Alt+KeyK' : 'Ctrl+KeyK' },
  { id: 'app.scm', label: 'source control', scope: 'app', combo: 'Ctrl+Shift+KeyG' },
  { id: 'app.themes', label: 'themes', scope: 'app', combo: 'Mod+Shift+Comma' },
  { id: 'app.shortcuts', label: 'shortcut settings', scope: 'app', combo: 'Mod+Comma' },
]

export type WheelMouse = 'ctrl+middle' | 'middle' | 'back' | 'forward' | 'none'

const KEY = 'ui.shortcuts'

interface Saved {
  combos: Record<string, string>
  wheelMouse: WheelMouse
}

function load(): Saved {
  try {
    const s = JSON.parse(localStorage.getItem(KEY) ?? 'null') as Saved | null
    return { combos: s?.combos ?? {}, wheelMouse: s?.wheelMouse ?? 'ctrl+middle' }
  } catch {
    return { combos: {}, wheelMouse: 'ctrl+middle' }
  }
}

/** Basılan tuşu birleşim metnine çevirir ("Mod+Shift+KeyD"). Yalnız değiştirici tuşsa null. */
export function comboOf(e: KeyboardEvent): string | null {
  if (['Meta', 'Control', 'Alt', 'Shift'].includes(e.key)) return null
  const parts: string[] = []
  const mod = isMac ? e.metaKey : e.ctrlKey
  if (mod) parts.push('Mod')
  if (isMac && e.ctrlKey) parts.push('Ctrl')
  if (!isMac && e.metaKey) parts.push('Meta')
  if (e.altKey) parts.push('Alt')
  if (e.shiftKey) parts.push('Shift')
  parts.push(e.code)
  return parts.join('+')
}

const KEY_LABEL: Record<string, string> = {
  BracketLeft: '[', BracketRight: ']', ArrowLeft: '←', ArrowRight: '→', ArrowUp: '↑', ArrowDown: '↓',
  Enter: '↩', Comma: ',', Period: '.', Space: 'Space', Backspace: '⌫', Escape: 'Esc', Slash: '/',
}

/** Kullanıcıya gösterim: Mac'te ⌃⌥⇧⌘, Windows'ta Ctrl+Alt+Shift. */
export function label(combo: string): string {
  const parts = combo.split('+')
  const key = parts.pop() ?? ''
  const k = KEY_LABEL[key] ?? key.replace(/^Key/, '').replace(/^Digit/, '')
  const has = (m: string) => parts.includes(m)
  if (isMac) {
    return `${has('Ctrl') ? '⌃' : ''}${has('Alt') ? '⌥' : ''}${has('Shift') ? '⇧' : ''}${has('Mod') ? '⌘' : ''}${k}`
  }
  return [has('Mod') && 'Ctrl', has('Meta') && 'Win', has('Alt') && 'Alt', has('Shift') && 'Shift', k].filter(Boolean).join('+')
}

class Shortcuts {
  saved = $state<Saved>(load())

  all = $derived(DEFAULTS.map((d) => ({ ...d, combo: this.saved.combos[d.id] ?? d.combo, isDefault: !this.saved.combos[d.id] })))

  constructor() {
    if (typeof window === 'undefined') return
    // Başka bir pencerede değişirse burada da güncellensin.
    window.addEventListener('storage', (e) => {
      if (e.key === KEY) this.saved = load()
    })
  }

  combo(id: string): string {
    return this.saved.combos[id] ?? DEFAULTS.find((d) => d.id === id)?.combo ?? ''
  }

  /** Bu tuş basışı hangi eylem (verilen kapsamlarda)? */
  match(e: KeyboardEvent, scopes: Scope[]): string | null {
    const c = comboOf(e)
    if (!c) return null
    return this.all.find((d) => scopes.includes(d.scope) && d.combo === c)?.id ?? null
  }

  set(id: string, combo: string) {
    this.saved.combos[id] = combo
    this.persist()
  }

  reset(id: string) {
    delete this.saved.combos[id]
    this.persist()
  }

  setWheelMouse(m: WheelMouse) {
    this.saved.wheelMouse = m
    this.persist()
  }

  /** Aynı birleşimi kullanan başka eylem (çakışma uyarısı için). */
  conflict(id: string, combo: string): string | null {
    const me = DEFAULTS.find((d) => d.id === id)
    return this.all.find((d) => d.id !== id && d.combo === combo && (d.scope === me?.scope || d.scope === 'app'))?.label ?? null
  }

  wheelMouseMatches(e: MouseEvent): boolean {
    switch (this.saved.wheelMouse) {
      case 'ctrl+middle':
        return e.button === 1 && e.ctrlKey
      case 'middle':
        return e.button === 1
      case 'back':
        return e.button === 3
      case 'forward':
        return e.button === 4
      default:
        return false
    }
  }

  private persist() {
    try {
      localStorage.setItem(KEY, JSON.stringify(this.saved))
    } catch {
      /* yalnızca bu oturum */
    }
  }
}

export const shortcuts = new Shortcuts()
