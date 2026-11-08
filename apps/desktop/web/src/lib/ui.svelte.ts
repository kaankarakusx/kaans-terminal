// Ana paneldeki açılır ekranlar: klasör seçici, kısayol çarkı, ayarlar.

/** Kod görüntüleyicide açık dosya */
export interface CodeTarget {
  root: string
  path: string
  /** Yeniden adlandırmada eski yol */
  orig: string | null
  /** Staged: HEAD ↔ dizin; değil: dizin ↔ çalışma ağacı */
  staged: boolean
  mode: 'diff' | 'file'
}

export type SettingsTab = 'interface' | 'integrations' | 'shortcuts' | 'about'

class Ui {
  picker = $state(false)
  wheel = $state<{ x: number; y: number } | null>(null)
  settings = $state(false)
  /** Source Control paneli (⌃⇧G) */
  scm = $state(false)
  /** Arkadaşlar (sol sütun) */
  friends = $state(false)
  /** Duyurular (sol sütun, zil) */
  news = $state(false)
  /** Geri bildirim penceresi */
  feedback = $state(false)

  /** Sol sütundaki görünümlerden birini aç/kapat (aynı anda biri) */
  side(which: 'scm' | 'friends' | 'news') {
    const open = !this[which]
    this.scm = which === 'scm' && open
    this.friends = which === 'friends' && open
    this.news = which === 'news' && open
  }
  /** Tema sayfası (ana alanı kaplar) */
  themes = $state(false)
  /** Sıralama sayfası (kupa) */
  board = $state(false)

  /** Ana alanı kaplayan sayfalardan birini aç/kapat (aynı anda biri) */
  page(which: 'themes' | 'board') {
    const open = which === 'themes' ? !this.themes : !this.board
    this.themes = which === 'themes' && open
    this.board = which === 'board' && open
  }
  /** Kod görüntüleyici (orta alan) */
  code = $state<CodeTarget | null>(null)
  settingsTab = $state<SettingsTab>('integrations')

  openSettings(tab: SettingsTab) {
    this.settingsTab = tab
    this.settings = true
  }
}

export const ui = new Ui()
