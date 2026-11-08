// Yakalanmayan hatalar yerel günlüğe (`<app_data>/errors.log`, Rust `report_error`); hiçbir yere gönderilmez.
// Aynı hata 10 sn içinde tekrar yazılmaz.
const seen = new Map<string, number>()

export function reportError(err: unknown, where = '') {
  const message = `${where ? `${where}: ` : ''}${err instanceof Error ? err.message : String(err)}`
  const stack = err instanceof Error ? err.stack : undefined
  console.error('[error]', message, err)
  if (!('__TAURI_INTERNALS__' in window)) return
  const now = Date.now()
  if (now - (seen.get(message) ?? 0) < 10_000) return
  seen.set(message, now)
  void import('@tauri-apps/api/core').then(({ invoke }) => invoke('report_error', { message, stack: stack ?? null })).catch(() => {})
}

/** Pencere düzeyinde yakalanmayan hatalar ve reddedilen sözler */
export function installErrorLog() {
  window.addEventListener('error', (e) => reportError(e.error ?? e.message, 'window'))
  window.addEventListener('unhandledrejection', (e) => reportError(e.reason, 'promise'))
}

/**
 * Donma bekçisi nabzı (Rust `ui_beat`, `watchdog.rs`): 2 sn'de bir. JS iş parçacığı donarsa nabız durur; pencere
 * görünür ve öndeyken 10 sn sessizlik günlüğe yazılır.
 */
export function installUiBeat() {
  if (!('__TAURI_INTERNALS__' in window)) return
  void import('@tauri-apps/api/core').then(({ invoke }) => {
    const beat = () => void invoke('ui_beat', { visible: document.visibilityState === 'visible' }).catch(() => {})
    beat()
    setInterval(beat, 2000)
    document.addEventListener('visibilitychange', beat)
  })
}
