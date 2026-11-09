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

export type SettingsTab = 'interface' | 'profiles' | 'integrations' | 'shortcuts' | 'about'

class Ui {
  picker = $state(false)
  wheel = $state<{ x: number; y: number } | null>(null)
  /** Ayarlar sayfası (ana alanı kaplar, tema/sıralama sayfası gibi) */
  settings = $state(false)
  /** Source Control paneli (⌃⇧G) */
  scm = $state(false)
  /** Arkadaşlar (sol sütun) */
  friends = $state(false)
  /** Duyurular (sol sütun, zil) */
  news = $state(false)
  /** Geri bildirim penceresi */
  feedback = $state(false)

  /** Sol sütundaki görünümlerden birini aç/kapat (aynı anda biri). Açılırken ana alanı kaplayan sayfa (tema,
   *  sıralama) kapanır: sayfa sol sütunun da üstünde durur, görünüm arkasında kalırdı. */
  side(which: 'scm' | 'friends' | 'news') {
    this.setSide(which, !this[which])
  }

  /** Yalnızca açar (açıksa açık kalır): bildirimdeki "aç" düğmeleri */
  show(which: 'scm' | 'friends' | 'news') {
    this.setSide(which, true)
  }

  private setSide(which: 'scm' | 'friends' | 'news', open: boolean) {
    this.scm = which === 'scm' && open
    this.friends = which === 'friends' && open
    this.news = which === 'news' && open
    if (open) {
      this.themes = false
      this.board = false
      this.settings = false
    }
  }
  /** Tema sayfası (ana alanı kaplar) */
  themes = $state(false)
  /** Sıralama sayfası (kupa) */
  board = $state(false)

  /** Ana alanı kaplayan sayfalardan birini aç/kapat (aynı anda biri) */
  page(which: 'themes' | 'board' | 'settings') {
    const open = !this[which]
    this.themes = which === 'themes' && open
    this.board = which === 'board' && open
    this.settings = which === 'settings' && open
  }
  /** Kod görüntüleyici (orta alan) */
  code = $state<CodeTarget | null>(null)
  settingsTab = $state<SettingsTab>('integrations')

  /** Ayarlar sayfasını verilen sekmede açar (açıksa sekmeye geçer) */
  openSettings(tab: SettingsTab) {
    this.settingsTab = tab
    if (!this.settings) this.page('settings')
  }
}

export const ui = new Ui()
