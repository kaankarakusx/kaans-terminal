<!--
  Sağ alt (OZELLIKLER.md §6.2): gelen mesaj köşede görünür ve oradan cevaplanır (uygulamayı öne getirmez, yazarken
  kutu kaybolmaz), ⤢ sohbeti açar. "is now online" ve yeni istek bildirimleri. Açık sohbetler yan yana kutular.
-->
<script lang="ts">
  import { tick } from 'svelte'
  import { clock } from './format'
  import RichText from './RichText.svelte'
  import { LOCKED_TEXT, friends, nameTag, type Ping } from './friends.svelte'
  import { news } from './news.svelte'
  import { ui } from './ui.svelte'

  let replies = $state<Record<string, string>>({})
  let drafts = $state<Record<string, string>>({})
  let errors = $state<Record<string, string | null>>({})
  let logs = $state<Record<string, HTMLElement | undefined>>({})

  async function reply(p: Ping) {
    const text = replies[p.key] ?? ''
    if (!text.trim()) return
    const err = await friends.send(nameTag(p.who), text)
    if (err) errors[p.key] = err
    else {
      replies[p.key] = ''
      friends.dismiss(p.key)
    }
  }

  async function send(k: string) {
    const err = await friends.send(k, drafts[k] ?? '')
    errors[k] = err
    if (!err) drafts[k] = ''
  }

  // Yeni mesaj gelince sohbetin sonuna kaydır
  $effect(() => {
    for (const k of friends.open) {
      void (friends.chats[k]?.length ?? 0)
      void tick().then(() => logs[k]?.scrollTo({ top: logs[k]!.scrollHeight }))
    }
  })

  const friendOf = (k: string) => friends.friends.find((f) => nameTag(f) === k)
</script>

<div class="dock" aria-live="polite">
  {#each news.fresh as a (a.id)}
    <div class="ping news">
      <div class="head">
        <span class="kind">announcement</span>
        <b>{a.title}</b>
        <span class="grow"></span>
        <button class="ic" title="open announcements" aria-label="open announcements" onclick={() => { ui.side('news'); news.dismiss(a.id) }}>⤢</button>
        <button class="ic" aria-label="dismiss" onclick={() => news.dismiss(a.id)}>×</button>
      </div>
      <p class="body"><RichText text={a.body} /></p>
    </div>
  {/each}
  {#each friends.pings as p (p.key)}
    <div class="ping {p.kind}">
      <div class="head">
        <span class="kind">{p.kind === 'message' ? 'message' : p.kind === 'online' ? 'online' : 'friend request'}</span>
        <b>{p.who.name}</b><span class="tag">#{p.who.tag}</span>
        <span class="grow"></span>
        {#if p.kind === 'message'}<button class="ic" title="open chat" aria-label="open chat" onclick={() => friends.openChat(p.who)}>⤢</button>{/if}
        <button class="ic" aria-label="dismiss" onclick={() => friends.dismiss(p.key)}>×</button>
      </div>
      {#if p.kind === 'message'}
        <p class="body">{p.body}</p>
        <form class="reply" onsubmit={(e) => { e.preventDefault(); void reply(p) }}>
          <input bind:value={replies[p.key]} placeholder="reply…" aria-label="reply to {p.who.name}" />
        </form>
        {#if errors[p.key]}<p class="err">✕ {errors[p.key]}</p>{/if}
      {:else if p.kind === 'online'}
        <p class="body">is now online</p>
      {:else}
        <p class="body">wants to be friends · <button class="link" onclick={() => { void friends.accept(p.who); friends.dismiss(p.key) }}>accept</button></p>
      {/if}
    </div>
  {/each}
</div>

<div class="chats">
  {#each friends.open as k (k)}
    {@const f = friendOf(k)}
    <section class="chat" aria-label="chat with {k}">
      <header>
        <span class="dot" class:online={f?.online && !f.away} class:away={f?.away}></span>
        <b>{k.split('#')[0]}</b><span class="tag">#{k.split('#')[1]}</span>
        <span class="grow"></span>
        <button class="ic" aria-label="close chat" onclick={() => friends.closeChat(k)}>×</button>
      </header>
      <div class="log" bind:this={logs[k]}>
        {#each friends.chats[k] ?? [] as m (m.id)}
          <div class="msg" class:mine={m.mine} class:locked={m.body == null} title={clock(m.at)}>{m.body ?? LOCKED_TEXT}</div>
        {:else}
          <p class="empty">Say hi 👋</p>
        {/each}
      </div>
      {#if errors[k]}<p class="err">✕ {errors[k]}</p>{/if}
      <form class="send" onsubmit={(e) => { e.preventDefault(); void send(k) }}>
        <textarea
          rows="2"
          bind:value={drafts[k]}
          placeholder="message… (Enter to send, Shift+Enter for a new line)"
          aria-label="message {k}"
          onkeydown={(e) => {
            if (e.key === 'Enter' && !e.shiftKey) {
              e.preventDefault()
              void send(k)
            }
          }}
        ></textarea>
      </form>
    </section>
  {/each}
</div>

<style>
  /* Konum App.svelte'deki .corner sütunundan */
  .dock { display: flex; flex-direction: column; gap: 6px; width: min(320px, calc(100vw - 32px)); pointer-events: none; }
  .dock:empty { display: none; }
  .ping { pointer-events: auto; padding: 8px 10px; background: rgb(var(--panel-rgb)); border: 1px solid var(--line); border-left: 3px solid var(--accent); box-shadow: 0 8px 24px rgba(0, 0, 0, 0.45); animation: in 0.2s ease-out; }
  .ping.online { border-left-color: var(--green); }
  .ping.request { border-left-color: var(--violet); }
  .ping.news { border-left-color: var(--cyan); }
  @keyframes in { from { transform: translateX(16px); opacity: 0; } }
  .head { display: flex; align-items: baseline; gap: 4px; }
  .kind { color: var(--dim); font-size: 0.8em; text-transform: uppercase; letter-spacing: 0.06em; margin-right: 4px; }
  .head b { color: var(--hi); }
  .tag { color: var(--dim); }
  .grow { flex: 1; }
  .ic { all: unset; padding: 0 4px; color: var(--dim); cursor: pointer; }
  .ic:hover, .ic:focus-visible { color: var(--hi); }
  .body { margin: 4px 0; color: var(--txt); white-space: pre-wrap; word-break: break-word; max-height: 6.5em; overflow: auto; }
  .reply input { all: unset; box-sizing: border-box; width: 100%; padding: 3px 6px; border: 1px solid var(--line); background: rgba(var(--bg-rgb), 0.6); color: var(--hi); }
  .reply input:focus { border-color: var(--accent); }
  .link { all: unset; color: var(--accent); cursor: pointer; text-decoration: underline; }
  .err { color: var(--red); margin: 4px 0 0; font-size: 0.9em; }

  .chats { position: fixed; right: 352px; bottom: 38px; z-index: 114; display: flex; gap: 8px; align-items: flex-end; }
  .chat { width: 300px; height: 380px; display: flex; flex-direction: column; background: rgb(var(--panel-rgb)); border: 1px solid var(--accent); box-shadow: 0 10px 30px rgba(0, 0, 0, 0.5); }
  .chat header { display: flex; align-items: baseline; gap: 4px; padding: 5px 8px; border-bottom: 1px solid var(--line-soft); }
  .chat header b { color: var(--hi); }
  .dot { align-self: center; width: 7px; height: 7px; border-radius: 50%; background: var(--ended); margin-right: 3px; }
  .dot.online { background: var(--green); box-shadow: 0 0 5px var(--green); }
  .dot.away { background: var(--amber); }
  .log { flex: 1; overflow: auto; padding: 8px; display: flex; flex-direction: column; gap: 4px; }
  .msg { align-self: flex-start; max-width: 82%; padding: 4px 8px; background: rgba(var(--accent-rgb), 0.08); border: 1px solid var(--line-soft); color: var(--txt); white-space: pre-wrap; word-break: break-word; user-select: text; }
  .msg.locked { color: var(--dim); font-style: italic; }
  .msg.mine { align-self: flex-end; background: rgba(var(--accent-rgb), 0.22); border-color: rgba(var(--accent-rgb), 0.5); }
  .empty { color: var(--faint); margin: auto; }
  .send textarea { all: unset; box-sizing: border-box; display: block; width: 100%; padding: 6px 8px; border-top: 1px solid var(--line-soft); background: rgba(var(--bg-rgb), 0.6); color: var(--hi); white-space: pre-wrap; }
  .send textarea::placeholder { color: var(--faint); }
  @media (prefers-reduced-motion: reduce) { .ping { animation: none; } }
  @media (max-width: 900px) { .chats { right: 12px; bottom: auto; top: 60px; } }
</style>
