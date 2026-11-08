<!--
  Bir arkadaşla sohbet penceresi (`#chat=name#tag`, OZELLIKLER.md §6.2): geçmiş (sayfalı), canlı gelen mesajlar,
  kopyala/yapıştır. Açıkken ana pencere bu arkadaşın mesajları için köşede bildirim göstermez (chatWindowOpen).
-->
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { rankOf } from '@kaans/shared/xp'
  import { onMount, tick } from 'svelte'
  import RankBadge from '../lib/RankBadge.svelte'
  import RichText from '../lib/RichText.svelte'
  import { inTauri } from '../lib/engine.svelte'
  import { clock } from '../lib/format'
  import { LOCKED_TEXT, markChatWindow, type ChatMessage, type Friend } from '../lib/friends.svelte'

  const k = decodeURIComponent(location.hash.replace(/^#chat=/, ''))
  const [name, tag] = k.split('#')
  const isMac = navigator.userAgent.includes('Mac')

  let messages = $state<ChatMessage[]>([])
  let more = $state(false)
  let friend = $state<Friend | null>(null)
  let draft = $state('')
  let error = $state<string | null>(null)
  let log = $state<HTMLElement>()
  let input = $state<HTMLTextAreaElement>()

  async function load(before?: string) {
    if (!inTauri) {
      messages = [
        { id: '1', mine: true, body: 'build geçti mi?', at: Date.now() - 3.6e6 },
        { id: '2', mine: false, body: 'geçti 🎉 PR: https://github.com/kaankarakusx/kaans-terminal/pull/12', at: Date.now() - 3.5e6 },
      ]
      return
    }
    try {
      const r = await invoke<{ messages: ChatMessage[]; more: boolean }>('social_get', { what: 'messages', with: k, before })
      messages = before ? [...r.messages, ...messages] : r.messages
      more = r.more
      error = null
    } catch (e) {
      error = String(e) === 'offline' ? "can't reach the server right now" : String(e)
    }
  }

  async function loadFriend() {
    if (!inTauri) {
      friend = { name, tag, level: 27, theme: null, online: true, away: false, todaySecs: 3900, last7: null, onFire: false, unread: 0 }
      return
    }
    try {
      const r = await invoke<{ friends: Friend[] }>('social_get', { what: 'friends' })
      friend = r.friends.find((f) => `${f.name}#${f.tag}` === k) ?? null
    } catch {
      /* çevrimdışı */
    }
  }

  async function bottom() {
    await tick()
    log?.scrollTo({ top: log.scrollHeight })
  }

  async function send() {
    const text = draft.trim()
    if (!text) return
    if (!inTauri) {
      messages = [...messages, { id: String(Date.now()), mine: true, body: text, at: Date.now() }]
      draft = ''
      void bottom()
      return
    }
    try {
      const r = await invoke<{ id: string; at: number }>('social_post', { what: 'messages.send', body: { to: k, body: text } })
      messages = [...messages, { id: r.id, mine: true, body: text, at: r.at }]
      draft = ''
      error = null
      void bottom()
    } catch (e) {
      error = String(e) === 'offline' ? "can't reach the server right now" : String(e)
    }
  }

  onMount(() => {
    document.title = k
    const stopMark = markChatWindow(k)
    void load().then(bottom)
    void loadFriend()
    input?.focus()
    let off: (() => void) | undefined
    if (inTauri) {
      let reread: ReturnType<typeof setTimeout> | undefined
      void import('@tauri-apps/api/event').then(({ listen }) =>
        listen<Record<string, unknown>>('social://live', (e) => {
          const ev = e.payload
          const from = ev.from as { name: string; tag: string } | undefined
          if (ev.type === 'message' && from && `${from.name}#${from.tag}` === k) {
            messages = [...messages, { id: String(ev.id), mine: false, body: ev.body == null ? null : String(ev.body), at: Number(ev.at), locked: !!ev.locked }]
            void bottom()
            // Sunucuda okundu işaretlensin (ana penceredeki okunmamış sayısı)
            clearTimeout(reread)
            reread = setTimeout(() => void invoke('social_get', { what: 'messages', with: k }).catch(() => {}), 1500)
          } else if (ev.type === 'presence' && `${ev.name}#${ev.tag}` === k && friend) {
            friend = { ...friend, online: !!ev.online, away: !!ev.away }
          }
        }).then((f) => (off = f)),
      )
    }
    return () => {
      stopMark()
      off?.()
    }
  })

  const rank = $derived(rankOf(friend?.level ?? 1))
  const status = $derived(friend ? (friend.online ? (friend.away ? 'away' : 'online') : 'offline') : '')
</script>

<div class="win" class:mac={isMac && inTauri}>
  <header data-tauri-drag-region>
    <span class="dot {status}" data-tauri-drag-region></span>
    {#if friend}<RankBadge {rank} size={20} />{/if}
    <b data-tauri-drag-region>{name}</b><span class="tag" data-tauri-drag-region>#{tag}</span>
    <span class="e2e" title="Messages are encrypted on this device; the server only stores ciphertext." data-tauri-drag-region>🔒 end-to-end</span>
    {#if friend}<span class="meta" data-tauri-drag-region>Lv {friend.level} · {status}</span>{/if}
  </header>

  <div class="log" bind:this={log}>
    {#if more}<button class="older" onclick={() => load(messages[0]?.id)}>load older messages</button>{/if}
    {#each messages as m, i (m.id)}
      {@const prev = messages[i - 1]}
      {#if !prev || m.at - prev.at > 15 * 60_000}<div class="time">{new Date(m.at).toLocaleString('en-GB', { dateStyle: 'medium', timeStyle: 'short' })}</div>{/if}
      {#if m.body == null}
        <div class="msg locked" class:mine={m.mine} title={clock(m.at)}>{LOCKED_TEXT}</div>
      {:else}
        <div class="msg" class:mine={m.mine} title={clock(m.at)}><RichText text={m.body} /></div>
      {/if}
    {:else}
      <p class="empty">Say hi to {name} 👋</p>
    {/each}
  </div>

  {#if error}<p class="err">✕ {error}</p>{/if}
  <textarea
    bind:this={input}
    bind:value={draft}
    rows="3"
    maxlength="2000"
    placeholder="message {name}… (Enter to send, Shift+Enter for a new line)"
    aria-label="message {k}"
    onkeydown={(e) => {
      if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) {
        e.preventDefault()
        void send()
      }
    }}
  ></textarea>
</div>

<style>
  :global(html), :global(body) { background: transparent; }
  .win { height: 100vh; display: flex; flex-direction: column; background: rgba(var(--term-bg-rgb), 0.97); border: 1px solid rgba(var(--accent-rgb), 0.4); border-radius: 10px; overflow: hidden; box-sizing: border-box; }
  header { flex: none; height: 34px; display: flex; align-items: center; gap: 7px; padding: 0 12px; background: rgba(var(--panel-rgb), 0.98); border-bottom: 1px solid var(--line-soft); user-select: none; -webkit-user-select: none; }
  .mac header { padding-left: 82px; }
  header b { color: var(--hi); }
  .tag { color: var(--dim); }
  .meta { color: var(--dim); }
  .e2e { margin-left: auto; color: var(--green); font-size: 0.85em; opacity: 0.8; }
  .msg.locked { color: var(--dim); font-style: italic; }
  .dot { width: 8px; height: 8px; border-radius: 50%; background: var(--ended); flex: none; }
  .dot.online { background: var(--green); box-shadow: 0 0 6px var(--green); }
  .dot.away { background: var(--amber); }
  .log { flex: 1; overflow: auto; padding: 10px 12px; display: flex; flex-direction: column; gap: 5px; }
  .older { all: unset; align-self: center; padding: 2px 10px; border: 1px solid var(--line); color: var(--dim); cursor: pointer; margin-bottom: 6px; }
  .older:hover { color: var(--hi); border-color: var(--accent); }
  .time { align-self: center; color: var(--faint); font-size: 0.85em; margin: 6px 0 2px; }
  .msg { align-self: flex-start; max-width: 82%; padding: 5px 9px; background: rgba(var(--accent-rgb), 0.08); border: 1px solid var(--line-soft); color: var(--txt); white-space: pre-wrap; word-break: break-word; user-select: text; -webkit-user-select: text; }
  .msg.mine { align-self: flex-end; background: rgba(var(--accent-rgb), 0.22); border-color: rgba(var(--accent-rgb), 0.5); }
  .empty { margin: auto; color: var(--faint); }
  .err { margin: 0; padding: 4px 12px; color: var(--red); }
  textarea { all: unset; box-sizing: border-box; flex: none; width: 100%; padding: 8px 12px; border-top: 1px solid var(--line); background: rgba(var(--bg-rgb), 0.7); color: var(--hi); white-space: pre-wrap; }
  textarea::placeholder { color: var(--faint); }
</style>
