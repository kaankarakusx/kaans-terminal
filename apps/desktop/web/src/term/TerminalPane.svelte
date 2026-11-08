<!--
  Tek bir terminal bölmesi: xterm.js + oturum sunucusuna bağlantı.
  Klavye davranışı iTerm2 gibi (OZELLIKLER.md §3.4); pencere kısayollarını TermApp yakalar.
-->
<script lang="ts" module>
  import type { Terminal } from '@xterm/xterm'
  import type { SearchAddon } from '@xterm/addon-search'

  /** TermApp'in öndeki bölmeye komut verdiği arayüz. */
  export interface PaneControl {
    term: Terminal
    search: SearchAddon
    session: string
    send: (data: string) => void
    focus: () => void
  }

  export const panes = new Map<string, PaneControl>()
</script>

<script lang="ts">
  import { onMount } from 'svelte'
  import { Terminal as XTerm } from '@xterm/xterm'
  import { FitAddon } from '@xterm/addon-fit'
  import { SearchAddon as Search } from '@xterm/addon-search'
  import { Unicode11Addon } from '@xterm/addon-unicode11'
  import { WebLinksAddon } from '@xterm/addon-web-links'
  import { openUrl } from '@tauri-apps/plugin-opener'
  import '@xterm/xterm/css/xterm.css'
  import { inTauri } from '../lib/engine.svelte'
  import * as api from './api'
  import { TERM_FONT, xtermTheme } from './xtermTheme'

  let {
    paneId,
    session,
    cwd,
    active,
    onfocus,
    isAppShortcut,
    oncontextmenu,
  }: {
    paneId: string
    session: string
    cwd: string | null
    active: boolean
    onfocus: () => void
    /** Pencere kısayolu mu (xterm bu tuşu terminale göndermesin) */
    isAppShortcut: (e: KeyboardEvent) => boolean
    oncontextmenu: (e: MouseEvent) => void
  } = $props()

  let host = $state<HTMLDivElement>()
  let ended = $state(false)

  /** Pencere zemini saydam mı (%90 opak pencere). */
  const TRANSPARENT = true

  // Dosya yolu: /mutlak, ~/ev, ./göreli, src/x.ts; isteğe bağlı :satır:sütun
  const PATH_RE = /(?:~\/|\.{1,2}\/|\/)?(?:[\w.@-]+\/)+[\w.@-]+\.\w+(?::\d+(?::\d+)?)?|(?:~\/|\/)[\w.@\/-]+/g

  onMount(() => {
    if (!host) return
    const term = new XTerm({
      fontFamily: TERM_FONT,
      fontSize: 13,
      lineHeight: 1.15,
      cursorBlink: true,
      cursorStyle: 'block',
      allowProposedApi: true,
      scrollback: 20_000,
      // Option tuşu karakter üretir (@ # { } Türkçe klavyede Option ile); kelime gezintisi aşağıda elle eşlenir.
      macOptionIsMeta: false,
      // ⌥-sürükle terminalde seçer (uygulama fareyi yakalasa da).
      macOptionClickForcesSelection: true,
      // Pencere %90 opak: xterm zemini saydam çizer, rengi pencere verir.
      allowTransparency: TRANSPARENT,
      theme: xtermTheme(TRANSPARENT),
    })
    const fit = new FitAddon()
    const search = new Search()
    term.loadAddon(fit)
    term.loadAddon(search)
    const unicode = new Unicode11Addon()
    term.loadAddon(unicode)
    term.unicode.activeVersion = '11'

    // ⌘-tık: bağlantı tarayıcıda açılır.
    term.loadAddon(
      new WebLinksAddon((e, uri) => {
        if (!(e.metaKey || e.ctrlKey)) return
        if (inTauri) void openUrl(uri)
        else window.open(uri, '_blank')
      }),
    )
    // ⌘-tık: dosya yolu kendi uygulamasında açılır. Satır kırılmış uzun yollar birleştirilerek aranır.
    term.registerLinkProvider({
      provideLinks(y, callback) {
        const buf = term.buffer.active
        let start = y - 1
        while (start > 0 && buf.getLine(start)?.isWrapped) start--
        let text = ''
        const rows: number[] = []
        for (let r = start; r < buf.length; r++) {
          const line = buf.getLine(r)
          if (!line || (r > start && !line.isWrapped)) break
          rows.push(text.length)
          text += line.translateToString(true).padEnd(term.cols, ' ').slice(0, term.cols)
        }
        const toPos = (i: number) => {
          const row = Math.floor(i / term.cols)
          return { x: (i % term.cols) + 1, y: start + row + 1 }
        }
        const links = [...text.matchAll(PATH_RE)]
          .filter((m) => !/^https?:/.test(text.slice(Math.max(0, (m.index ?? 0) - 8), m.index)))
          .map((m) => {
            const s = toPos(m.index ?? 0)
            const e = toPos((m.index ?? 0) + m[0].length - 1)
            return {
              range: { start: s, end: e },
              text: m[0],
              activate: (ev: MouseEvent, t: string) => {
                if (ev.metaKey || ev.ctrlKey) void api.openPath(t, cwd)
              },
            }
          })
          .filter((l) => l.range.start.y <= y && l.range.end.y >= y)
        callback(links.length ? links : undefined)
      },
    })

    term.open(host)
    // Pencere %90 opak: zemin saydam çizilmeli. xterm'in WebGL çizicisi saydam zemini desteklemez (boyamadığı
    // yeri siyah bırakır), bu yüzden saydamken xterm'in kendi (DOM) çizicisi kullanılır. WebGL yalnızca opak
    // zeminde (TRANSPARENT = false) devreye girer.
    if (!TRANSPARENT) {
      void import('@xterm/addon-webgl')
        .then(({ WebglAddon }) => {
          const gl = new WebglAddon()
          gl.onContextLoss(() => gl.dispose())
          term.loadAddon(gl)
        })
        .catch(() => {})
    }

    let attachId: number | null = null
    const send = (data: string) => {
      if (attachId !== null && !ended) void api.input(attachId, data)
    }

    term.attachCustomKeyEventHandler((e) => {
      if (e.type !== 'keydown') return true
      if (isAppShortcut(e)) return false
      const mac = navigator.platform.toLowerCase().includes('mac')
      // Shift+Return: gönderme, yeni satır aç (Claude Code ESC+CR'yi yeni satır sayar).
      if (e.key === 'Enter' && e.shiftKey && !e.metaKey && !e.ctrlKey && !e.altKey) {
        send('\x1b\r')
        return false
      }
      if (mac && e.altKey && !e.metaKey && !e.ctrlKey) {
        if (e.key === 'ArrowLeft') return send('\x1bb'), false
        if (e.key === 'ArrowRight') return send('\x1bf'), false
        if (e.key === 'Backspace') return send('\x1b\x7f'), false
      }
      return true
    })
    term.onData(send)
    term.onBinary((d) => send(d))

    // Yapıştırma: resim (ekran görüntüsü, Finder'dan kopyalanan dosya) varsa Claude'a Ctrl+V gönderilir;
    // Claude resmi pano'dan kendisi okur. Metin xterm'in bracketed paste'iyle tek parça gider.
    const onPaste = (e: ClipboardEvent) => {
      const items = [...(e.clipboardData?.items ?? [])]
      if (items.some((i) => i.type.startsWith('image/') || i.kind === 'file')) {
        e.preventDefault()
        e.stopPropagation()
        send('\x16')
      }
    }
    host.addEventListener('paste', onPaste, true)
    host.addEventListener('mousedown', () => onfocus())

    const ctl: PaneControl = { term, search, session, send, focus: () => term.focus() }
    panes.set(paneId, ctl)

    const ro = new ResizeObserver(() => {
      try {
        fit.fit()
      } catch {
        /* görünmezken ölçülemez */
      }
    })
    ro.observe(host)
    term.onResize(({ cols, rows }) => {
      if (attachId !== null) void api.resize(attachId, cols, rows)
    })

    const onTheme = () => (term.options.theme = xtermTheme(TRANSPARENT))
    window.addEventListener('themechange', onTheme)

    let disposed = false
    void api
      .attach(session, (bytes) => term.write(bytes))
      .then((id) => {
        if (disposed) {
          void api.detach(id)
          return
        }
        attachId = id
        fit.fit()
        void api.resize(id, term.cols, term.rows)
      })
      .catch((e) => {
        ended = true
        term.write(`\r\n\x1b[31m[could not attach: ${e}]\x1b[0m\r\n`)
      })

    return () => {
      disposed = true
      ro.disconnect()
      window.removeEventListener('themechange', onTheme)
      host?.removeEventListener('paste', onPaste, true)
      panes.delete(paneId)
      // Bölme kapanır, oturum yaşar.
      if (attachId !== null) void api.detach(attachId)
      term.dispose()
    }
  })

  $effect(() => {
    if (active) panes.get(paneId)?.focus()
  })
</script>

<div class="pane" class:active>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="xterm-host" bind:this={host} oncontextmenu={oncontextmenu}></div>
</div>

<style>
  .pane {
    position: relative;
    height: 100%;
    width: 100%;
    min-width: 0;
    min-height: 0;
    background: transparent;
    padding: 4px 0 0 6px;
    box-sizing: border-box;
    outline: 1px solid transparent;
    outline-offset: -1px;
  }
  .pane.active { outline-color: rgba(var(--accent-rgb), 0.35); }
  .xterm-host { height: 100%; width: 100%; }
  /* Saydam zeminde xterm'in kendi kaydırma alanı da saydam; kaydırma çubuğu temadan. */
  .xterm-host :global(.xterm-viewport) {
    background-color: transparent !important;
    scrollbar-width: thin;
    scrollbar-color: rgba(var(--accent-rgb), 0.35) transparent;
  }
  .xterm-host :global(.xterm-screen),
  .xterm-host :global(.xterm-rows) { background: transparent; }
</style>
