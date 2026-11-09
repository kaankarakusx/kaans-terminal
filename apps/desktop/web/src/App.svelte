<script lang="ts">
  import { onMount } from 'svelte'
  import ConfirmDialog from './lib/ConfirmDialog.svelte'
  import ContextMenu from './lib/ContextMenu.svelte'
  import Danger from './lib/Danger.svelte'
  import FolderPicker from './lib/FolderPicker.svelte'
  import SettingsPage from './lib/SettingsPage.svelte'
  import SourceControl from './lib/SourceControl.svelte'
  import ActivityBar from './lib/ActivityBar.svelte'
  import ThemesPage from './lib/ThemesPage.svelte'
  import { reportError } from './lib/errors'
  import LeaderboardPage from './lib/LeaderboardPage.svelte'
  import Celebration from './lib/Celebration.svelte'
  import { xp } from './lib/xp.svelte'
  import HackerName from './lib/HackerName.svelte'
  import FriendsView from './lib/FriendsView.svelte'
  import NewsView from './lib/NewsView.svelte'
  import FeedbackDialog from './lib/FeedbackDialog.svelte'
  import WhatsNew from './lib/WhatsNew.svelte'
  import { updates } from './lib/updates.svelte'
  import { news } from './lib/news.svelte'
  import ChatDock from './lib/ChatDock.svelte'
  import { friends } from './lib/friends.svelte'
  import { social } from './lib/social.svelte'
  import { themes } from './themes/store.svelte'
  import CodeView from './lib/code/CodeView.svelte'
  import { git } from './lib/git.svelte'
  import ShortcutWheel from './lib/ShortcutWheel.svelte'
  import { runAppAction } from './lib/actions'
  import { shortcuts } from './lib/shortcuts.svelte'
  import { ui } from './lib/ui.svelte'
  import PanelArea from './lib/PanelArea.svelte'
  import Splash from './lib/Splash.svelte'
  import Ticker from './lib/Ticker.svelte'
  import Toasts from './lib/Toasts.svelte'
  import TopBar from './lib/TopBar.svelte'
  import { engine, inTauri } from './lib/engine.svelte'
  import { layout, type Area } from './lib/layout.svelte'
  import './lib/notify.svelte'
  import OrbitMap from './lib/orbit/OrbitMap.svelte'
  import './lib/power.svelte'

  engine.start().catch((e) => console.error('[engine]', e))

  // Önceki çalıştırmadan kalan terminal pencereleri (oturumları yaşıyorsa) geri gelir.
  onMount(() => {
    git.start()
    void xp.start()
    void social.start().catch((e) => console.warn('[social]', e))
    void updates.start().catch((e) => console.warn('[updates]', e))
    // Tarayıcı demosunda kutlamaları denemek için: __xp.preview('rank')
    if (!inTauri) (window as unknown as { __xp: typeof xp }).__xp = xp
    // Terminal pencereleri açılışta kendiliğinden gelmez: oturumlar yaşar, kart/araba ile açılır (düzeni hatırlanır).
    // İlk açılış: Entegrasyonlar ekranı bir kez gösterilir (hiçbir şey kendiliğinden açılmaz).
    try {
      if (!localStorage.getItem('ui.integrationsSeen')) {
        localStorage.setItem('ui.integrationsSeen', '1')
        ui.openSettings('integrations')
      }
    } catch {
      /* depolama yoksa gösterilmez */
    }
    // Mac'te dört parmak tıklaması (Rust tarafı yakalar) → kısayol çarkı.
    let off: (() => void) | undefined
    if (inTauri) {
      void import('@tauri-apps/api/event').then(({ listen }) =>
        listen('wheel://open', () => (ui.wheel = { ...pointer })).then((f) => (off = f)),
      )
    }
    return () => off?.()
  })

  let pointer = { x: innerWidth / 2, y: innerHeight / 2 }

  // Kayıt olunca arkadaşlar ve canlı olaylar
  $effect(() => {
    if (social.state.registered) {
      void friends.start()
      void news.start()
    }
  })

  // Seçili tema sunucuya bildirilir (sıralamada "in their theme")
  $effect(() => social.setTheme(themes.id))

  // On fire: bugün 5 saat aktif → ekran kenarlarında alevler (OZELLIKLER.md §6.3)
  $effect(() => {
    document.documentElement.classList.toggle('onfire', xp.onFire)
  })

  // Boş alan ızgaradan çıkar; panel taşınırken bırakma yeri olarak geri gelir.
  const has = (a: Area) => layout.visible(a).length > 0 || !!layout.drag
  const grid = $derived.by(() => {
    // Source Control sol sütunu kullanır (sohbet listesinin yerine)
    const l = has('left') || ui.scm || ui.friends || ui.news
    const r = has('right')
    const b = has('bottom')
    const s = layout.s.sizes
    const size = (a: Area, on: boolean) =>
      layout.visible(a).length || (a === 'left' && (ui.scm || ui.friends || ui.news)) ? `${s[a]}%` : on ? '120px' : ''
    // İlk sütun activity bar (üst bar ve alt şerit tam genişlik)
    const cols = ['44px', l && size('left', l), 'minmax(0, 1fr)', r && size('right', r)].filter(Boolean).join(' ')
    const row = (center: string, right: string) => ['act', l && 'left', center, r && right].filter(Boolean).join(' ')
    const full = (name: string) => [name, l && name, name, r && name].filter(Boolean).join(' ')
    const areas = [full('top'), row('map', 'right'), b && row('bottom', 'bottom'), full('ticker')]
      .filter(Boolean)
      .map((x) => `'${x}'`)
      .join(' ')
    const rows = ['auto', 'minmax(0, 1fr)', b && (layout.visible('bottom').length ? `${s.bottom}%` : '90px'), '22px']
      .filter(Boolean)
      .join(' ')
    return { cols, rows, areas, l, r, b }
  })

  // Uygulama kısayolları (⌘B sohbet listesi, ⌘N yeni terminal, çark, ayarlar); SHORTCUTS'tan değişir.
  function onkeydown(e: KeyboardEvent) {
    if (ui.picker) return
    const id = shortcuts.match(e, ['app'])
    if (!id) return
    e.preventDefault()
    if (id === 'app.wheel') ui.wheel = ui.wheel ? null : { ...pointer }
    else runAppAction(id)
  }

  // Kısayol çarkının fare tetikleyicisi (varsayılan ⌃ + orta tuş).
  function onmousedown(e: MouseEvent) {
    if (shortcuts.wheelMouseMatches(e)) {
      e.preventDefault()
      ui.wheel = { x: e.clientX, y: e.clientY }
    }
  }
</script>

<svelte:window {onkeydown} {onmousedown} onpointermove={(e) => (pointer = { x: e.clientX, y: e.clientY })} />

{#if !engine.snapshot}
  <Splash status="indexing transcripts…" />
{:else}
  <div
    class="app"
    class:moving={!!layout.drag}
    style:grid-template-columns={grid.cols}
    style:grid-template-rows={grid.rows}
    style:grid-template-areas={grid.areas}
  >
    <TopBar />
    <ActivityBar />
    {#if grid.l}<div class="cell" style:grid-area="left">{#if ui.scm}<SourceControl />{:else if ui.friends}<FriendsView />{:else if ui.news}<NewsView />{:else}<PanelArea area="left" />{/if}</div>{/if}
    <!-- Harita bir hata atarsa günlüğe yazılır ve 1 sn sonra yeniden kurulur (donup kalmaz) -->
    <div class="cell" style:grid-area="map">
      <svelte:boundary onerror={(e, reset) => { reportError(e, 'orbit.map'); setTimeout(reset, 1000) }}>
        <OrbitMap />
      </svelte:boundary>
    </div>
    {#if grid.r}<div class="cell" style:grid-area="right"><PanelArea area="right" /></div>{/if}
    {#if grid.b}<div class="cell" style:grid-area="bottom"><PanelArea area="bottom" /></div>{/if}
    <Ticker />
    <!-- Tema sayfası: activity bar, üst bar ve alt şerit dışındaki her yeri kaplar -->
    {#if ui.themes}<div class="cell over"><ThemesPage /></div>
    {:else if ui.board}<div class="cell over"><LeaderboardPage /></div>
    {:else if ui.settings}<div class="cell over"><SettingsPage /></div>
    {:else if ui.code}<div class="cell over" style:grid-column="{grid.l ? 3 : 2} / -1"><CodeView target={ui.code} /></div>{/if}
  </div>

  {#if layout.drag}
    <div class="ghost" style:left="{layout.drag.x + 12}px" style:top="{layout.drag.y + 8}px">{layout.drag.id}</div>
  {/if}
  <!-- Sağ alt köşe: mesaj bildirimleri ve uygulama uyarıları tek yığın -->
  <div class="corner">
    {#if social.state.registered}<ChatDock />{/if}
    <Toasts />
  </div>
  <Danger />
  <ContextMenu />
  <FolderPicker />
  <ShortcutWheel />
  <Celebration />
  {#if social.picking}<HackerName />{/if}
  {#if ui.feedback}<FeedbackDialog />{/if}
  <WhatsNew />
  <ConfirmDialog />
{/if}

<style>
  .app {
    height: 100%;
    display: grid;
    gap: 8px;
    padding: 8px;
  }
  .app.moving { cursor: grabbing; user-select: none; }
  .app > :global(.topbar) { grid-area: top; }
  .app > :global(.ticker) { grid-area: ticker; }
  .cell { min-height: 0; min-width: 0; }
  .corner {
    position: fixed;
    right: 16px;
    bottom: 38px;
    z-index: 115;
    display: flex;
    flex-direction: column;
    gap: 6px;
    pointer-events: none;
  }
  .over { grid-row: 2 / -2; grid-column: 2 / -1; z-index: 5; }

  .ghost {
    position: fixed;
    z-index: 90;
    padding: 2px 8px;
    background: var(--accent);
    color: var(--ink);
    font-weight: 700;
    opacity: 0.85;
    pointer-events: none;
    box-shadow: 0 0 14px rgba(var(--accent-rgb), 0.6);
  }

  /* Dar pencere: her şey tek sütuna iner (TASARIM.md §4.1). */
  @media (max-width: 900px) {
    .app {
      height: auto;
      min-height: 100%;
      grid-template-columns: 1fr !important;
      grid-template-rows: none !important;
      grid-template-areas: none !important;
      grid-auto-rows: auto;
    }
    .app > :global(*) { grid-area: auto !important; }
    .app > :global(.act) { flex-direction: row; justify-content: flex-start; }
    .cell { height: auto; }
    .cell :global(.area) { height: auto; }
    .cell :global(.slot) { height: 250px; flex: none !important; }
    .cell:has(:global(.map)) { height: 320px; }
    .app > :global(.topbar) { flex-wrap: wrap; }
    :global(html), :global(body) { overflow: auto; }
  }
</style>
