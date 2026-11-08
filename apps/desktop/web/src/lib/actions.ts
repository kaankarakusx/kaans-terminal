// Ana paneldeki eylemler: kısayollar ve kısayol çarkı aynı listeyi kullanır.
import { engine } from './engine.svelte'
import { layout } from './layout.svelte'
import { notifier } from './notify.svelte'
import { openChat } from './terminals'
import { ui } from './ui.svelte'

export interface Action {
  id: string
  label: string
  icon: string
  run: () => void
}

export function appActions(): Action[] {
  return [
    { id: 'app.newTerminal', label: 'new terminal', icon: '+', run: () => (ui.picker = true) },
    { id: 'app.toggleSessions', label: 'chat list', icon: '☰', run: () => layout.toggle('sessions') },
    {
      id: 'app.openWaiting',
      label: 'waiting chat',
      icon: '●',
      run: () => {
        const s = engine.snapshot?.sessions.find((x) => x.status === 'waiting') ??
          engine.snapshot?.sessions.find((x) => x.status === 'idle')
        if (s) void openChat(s)
      },
    },
    { id: 'app.sound', label: notifier.sound ? 'sound off' : 'sound on', icon: '♪', run: () => notifier.toggleSound() },
    { id: 'app.resetLayout', label: 'reset panels', icon: '▦', run: () => layout.reset() },
    { id: 'app.scm', label: 'source control', icon: '⑂', run: () => (ui.scm = !ui.scm) },
    { id: 'app.themes', label: 'themes', icon: '◐', run: () => ui.page('themes') },
    { id: 'app.shortcuts', label: 'settings', icon: '⚙', run: () => ui.openSettings('shortcuts') },
  ]
}

export function runAppAction(id: string) {
  appActions().find((a) => a.id === id)?.run()
}
