<!--
  Activity bar (TASARIM.md §4.1, VS Code gibi): sol kenarda görünümler. Sohbet listesi ve Source Control sol sütunu
  paylaşır; temalar ve ayarlar pencere açar. Arkadaşlar/sıralama/duyurular sunucu aşamasında eklenecek.
-->
<script lang="ts">
  import { friends } from './friends.svelte'
  import { git } from './git.svelte'
  import { news } from './news.svelte'
  import { layout } from './layout.svelte'
  import { label, shortcuts } from './shortcuts.svelte'
  import { ui } from './ui.svelte'

  const sessionsOn = $derived(!ui.scm && !ui.friends && !ui.news && !ui.themes && !ui.board && !ui.settings && !ui.servers && !layout.isHidden('sessions'))

  /** Başka bir görünüm (tema sayfası, Source Control) açıksa onu kapatıp listeyi gösterir; değilse listeyi aç/kapat. */
  function sessions() {
    if (ui.scm || ui.friends || ui.news || ui.themes || ui.board || ui.settings || ui.servers) {
      ui.scm = false
      ui.friends = false
      ui.news = false
      ui.themes = false
      ui.board = false
      ui.settings = false
      ui.servers = false
      layout.show('sessions')
    } else layout.toggle('sessions')
  }
</script>

<nav class="act" aria-label="views">
  <button class:on={sessionsOn} aria-pressed={sessionsOn} title="chats ({label(shortcuts.combo('app.toggleSessions'))})" aria-label="chats" onclick={sessions}>
    <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M14 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z" /><path d="M14 3v6h6" /><path d="M8 13h8M8 17h5" /></svg>
  </button>
  <button class:on={ui.scm} aria-pressed={ui.scm} title="source control ({label(shortcuts.combo('app.scm'))})" aria-label="source control" onclick={() => ui.side('scm')}>
    <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="6" cy="5" r="2.2" /><circle cx="6" cy="19" r="2.2" /><circle cx="18" cy="8" r="2.2" /><path d="M6 7.2v9.6" /><path d="M18 10.2c0 4-6 3.5-11.2 7" /></svg>
    {#if git.count}<span class="dot" aria-label="{git.count} changed files">{git.count > 99 ? '99+' : git.count}</span>{/if}
  </button>
  <button class:on={ui.servers} aria-pressed={ui.servers} title="servers (ssh)" aria-label="servers" onclick={() => ui.page('servers')}>
    <svg viewBox="0 0 24 24" aria-hidden="true"><rect x="3" y="4" width="18" height="7" rx="1" /><rect x="3" y="13" width="18" height="7" rx="1" /><path d="M7 7.5h.01M7 16.5h.01M11 7.5h6M11 16.5h6" /></svg>
  </button>
  <button class:on={ui.friends} aria-pressed={ui.friends} title="friends & messages" aria-label="friends" onclick={() => ui.side('friends')}>
    <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="9" cy="8" r="3.2" /><path d="M3 20c0-3.6 2.7-6 6-6s6 2.4 6 6" /><circle cx="17" cy="9" r="2.6" /><path d="M16 14.2c3 .2 5 2.4 5 5.3" /></svg>
    {#if friends.unread}<span class="dot" aria-label="{friends.unread} unread">{friends.unread > 99 ? '99+' : friends.unread}</span>{/if}
  </button>
  <button class:on={ui.board} aria-pressed={ui.board} title="leaderboard & profile" aria-label="leaderboard" onclick={() => ui.page('board')}>
    <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M8 21h8M12 17v4M7 4h10v5a5 5 0 0 1-10 0z" /><path d="M7 6H4v1a3 3 0 0 0 3 3M17 6h3v1a3 3 0 0 1-3 3" /></svg>
  </button>
  <button class:on={ui.themes} aria-pressed={ui.themes} title="themes ({label(shortcuts.combo('app.themes'))})" aria-label="themes" onclick={() => ui.page('themes')}>
    <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 3a9 9 0 1 0 0 18c1.1 0 1.6-.8 1.6-1.6 0-.5-.2-.9-.5-1.2-.3-.3-.5-.7-.5-1.2 0-.9.7-1.6 1.6-1.6H16a5 5 0 0 0 5-5c0-4-4-7.4-9-7.4z" /><circle cx="7.5" cy="11.5" r="1.1" /><circle cx="10.5" cy="7.5" r="1.1" /><circle cx="15.5" cy="8" r="1.1" /></svg>
  </button>
  <button class:on={ui.news} aria-pressed={ui.news} title="announcements" aria-label="announcements" onclick={() => ui.side('news')}>
    <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M6 16V11a6 6 0 0 1 12 0v5l1.5 2h-15z" /><path d="M10 20.5a2 2 0 0 0 4 0" /></svg>
    {#if news.unread}<span class="dot" aria-label="{news.unread} new">{news.unread}</span>{/if}
  </button>
  <span class="grow"></span>
  <button class="fb" title="send feedback to the developer" aria-label="feedback" onclick={() => (ui.feedback = true)}>
    <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 5h16v11H9l-5 4z" /><path d="M8 9h8M8 12h5" /></svg>
  </button>
  <button class:on={ui.settings} aria-pressed={ui.settings} title="settings ({label(shortcuts.combo('app.shortcuts'))})" aria-label="settings" onclick={() => ui.page('settings')}>
    <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="3" /><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z" /></svg>
  </button>
</nav>

<style>
  .act {
    grid-area: act;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 6px 0;
    background: rgba(var(--panel-rgb), 0.9);
    border: 1px solid var(--line-soft);
    min-height: 0;
  }
  .grow { flex: 1; }
  button {
    all: unset;
    position: relative;
    width: 36px;
    height: 36px;
    display: grid;
    place-items: center;
    color: var(--dim);
    cursor: pointer;
    border: 1px solid transparent;
  }
  button:hover, button:focus-visible { color: var(--hi); }
  button:focus-visible { border-color: var(--accent); }
  button.on {
    color: var(--accent);
    border-color: rgba(var(--accent-rgb), 0.6);
    background: rgba(var(--accent-rgb), 0.1);
    box-shadow: 0 0 10px rgba(var(--accent-rgb), 0.25);
  }
  button.on::before {
    content: '';
    position: absolute;
    left: -4px;
    top: 6px;
    bottom: 6px;
    width: 2px;
    background: var(--accent);
    box-shadow: 0 0 6px var(--accent);
  }
  /* Geri bildirim: hafifçe parlayan buton */
  .fb { color: var(--accent); animation: glow 3s ease-in-out infinite; }
  @keyframes glow { 50% { filter: drop-shadow(0 0 6px rgba(var(--accent-rgb), 0.9)); } }
  @media (prefers-reduced-motion: reduce) { .fb { animation: none; } }
  svg { width: 20px; height: 20px; fill: none; stroke: currentColor; stroke-width: 1.6; stroke-linecap: round; stroke-linejoin: round; }
  .dot {
    position: absolute;
    right: -2px;
    bottom: 0;
    min-width: 14px;
    padding: 0 3px;
    box-sizing: border-box;
    background: var(--accent);
    color: var(--ink);
    font-size: 0.75em;
    font-weight: 700;
    line-height: 14px;
    text-align: center;
  }
</style>
