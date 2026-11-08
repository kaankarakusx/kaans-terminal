import { mount } from 'svelte'
import './app.css'
// İmza tema yazı tipi (SIL OFL 1.1, pakette): yalnızca imza tema etiketlerde kullanınca yüklenir
import '@fontsource/space-grotesk/700.css'
import './themes/signature.css'
import App from './App.svelte'
import TermApp from './term/TermApp.svelte'
import ChatApp from './chat/ChatApp.svelte'
import { applyTokens } from './themes/apply'
import { deriveTheme } from './themes/derive'
import { themes } from './themes/store.svelte'
import { installErrorLog, installUiBeat } from './lib/errors'

// Token'lar bileşenler bağlanmadan önce yazılır: seçili tema (localStorage) + yazı tipi/boyutu. Varsayılan
// temanın değerleri tokens.css'te de var; açılışta renk sıçramaz.
themes.init()

// Yakalanmayan hatalar yerel günlüğe (`<app_data>/errors.log`)
installErrorLog()
installUiBeat()
window.__theme = (theme) => applyTokens(deriveTheme(theme))

// Aynı sayfa üç rolde: ana panel, terminal penceresi (`#term=<etiket>`) ya da arkadaşla sohbet (`#chat=name#tag`).
const isTerm = location.hash.startsWith('#term=')
const isChat = location.hash.startsWith('#chat=')
// Terminal ve sohbet pencereleri saydamdır (zemini kendileri boyar).
if (isTerm || isChat) document.documentElement.classList.add('term-window')
const app = mount(isTerm ? TermApp : isChat ? ChatApp : App, {
  target: document.getElementById('app')!,
})

export default app
