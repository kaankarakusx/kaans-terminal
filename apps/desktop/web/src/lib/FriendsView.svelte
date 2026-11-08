<!--
  Arkadaşlar (OZELLIKLER.md §6.2): sol sütunda, sohbet listesinin yerine (activity bar). name#tag ile ekleme, gelen ve
  giden istekler, arkadaşlar (durum, level, bugün, son 7 gün, okunmamış). Bir arkadaşa tıklamak sohbeti açar.
-->
<script lang="ts">
  import { rankOf } from '@kaans/shared/xp'
  import RankBadge from './RankBadge.svelte'
  import { duration } from './format'
  import { friends, nameTag, type Presence } from './friends.svelte'
  import { social } from './social.svelte'
  import { ui } from './ui.svelte'

  let add = $state('')
  let addError = $state<string | null>(null)
  let addOk = $state<string | null>(null)
  let busy = $state(false)

  $effect(() => {
    void friends.load()
  })

  async function submit(e: SubmitEvent) {
    e.preventDefault()
    if (busy || !add.trim()) return
    busy = true
    addError = null
    addOk = null
    const target = add.trim()
    addError = await friends.add(target)
    if (!addError) {
      addOk = `request sent to ${target}`
      add = ''
    }
    busy = false
  }

  const sorted = $derived(
    [...friends.friends].sort((a, b) => Number(b.online) - Number(a.online) || b.unread - a.unread || a.name.localeCompare(b.name)),
  )
  const PRESENCE: { v: Presence; l: string }[] = [
    { v: 'online', l: 'online' },
    { v: 'away', l: 'away' },
    { v: 'invisible', l: 'invisible' },
  ]

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && !e.defaultPrevented && friends.open.length === 0) ui.friends = false
  }
</script>

<svelte:window {onkeydown} />

<aside class="fv" aria-label="friends">
  <header>
    <span class="label">friends</span>
    <span class="sub">{friends.friends.filter((f) => f.online).length} online</span>
    <span class="grow"></span>
    <span class="conn" class:on={friends.connected} title={friends.connected ? 'live connection open' : 'offline: reconnecting'}></span>
    <button class="ic" title="back to chats (Esc)" aria-label="close" onclick={() => (ui.friends = false)}>×</button>
  </header>

  {#if !social.state.registered}
    <div class="empty">
      <p>Pick a hacker name to add friends and chat.</p>
      <button class="cta" onclick={() => (social.picking = true)}>choose a hacker name</button>
    </div>
  {:else}
    <div class="body">
      <div class="me">
        <b>{social.state.name}<span class="tag">#{social.state.tag}</span></b>
        <span class="grow"></span>
        <span class="seg" role="radiogroup" aria-label="your status">
          {#each PRESENCE as p (p.v)}
            <button role="radio" aria-checked={friends.presence === p.v} class:on={friends.presence === p.v} class={p.v} onclick={() => friends.setPresence(p.v)}>{p.l}</button>
          {/each}
        </span>
      </div>

      <form class="add" onsubmit={submit}>
        <input bind:value={add} placeholder="add a friend: name#tag" aria-label="friend's name#tag" spellcheck="false" autocomplete="off" />
        <button type="submit" disabled={busy || !add.trim()}>+ add</button>
      </form>
      {#if addError}<p class="err" role="alert">✕ {addError}</p>{/if}
      {#if addOk}<p class="ok">✓ {addOk}</p>{/if}

      {#if friends.incoming.length}
        <h3>requests <span class="count">{friends.incoming.length}</span></h3>
        <ul>
          {#each friends.incoming as p (nameTag(p))}
            <li class="req">
              <span class="who"><b>{p.name}</b><span class="tag">#{p.tag}</span></span>
              <button class="yes" onclick={() => friends.accept(p)}>accept</button>
              <button class="no" title="decline" aria-label="decline {nameTag(p)}" onclick={() => friends.remove(p)}>×</button>
            </li>
          {/each}
        </ul>
      {/if}

      <h3>friends <span class="count">{friends.friends.length}</span></h3>
      {#if sorted.length === 0}
        <p class="note">No friends yet. Share your name: <b>{social.state.name}#{social.state.tag}</b></p>
      {/if}
      <ul>
        {#each sorted as f (nameTag(f))}
          {@const max = Math.max(3600, ...(f.last7 ?? []))}
          <li class="friend" class:off={!f.online} class:fire={f.onFire}>
            <button class="row" title="message {nameTag(f)}" onclick={() => friends.openChat(f)}>
              <span class="dot" class:online={f.online && !f.away} class:away={f.away} aria-label={f.online ? (f.away ? 'away' : 'online') : 'offline'}></span>
              <RankBadge rank={rankOf(f.level)} size={22} />
              <span class="who">
                <span class="line"><b>{f.name}</b><span class="tag">#{f.tag}</span><span class="lv">Lv {f.level}</span></span>
                <small>{f.online ? (f.away ? 'away' : 'online') : 'offline'}{f.todaySecs != null ? ` · ${duration(f.todaySecs)} today` : ''}{f.onFire ? ' · 🔥' : ''}</small>
              </span>
              <span class="spark" aria-hidden="true">
                {#each f.last7 ?? [] as s, k (k)}<i style:height="{s ? 12 + (88 * s) / max : 4}%"></i>{/each}
              </span>
              {#if f.unread}<span class="unread" aria-label="{f.unread} unread">{f.unread}</span>{/if}
            </button>
          </li>
        {/each}
      </ul>

      {#if friends.outgoing.length}
        <h3>sent</h3>
        <ul>
          {#each friends.outgoing as p (nameTag(p))}
            <li class="req">
              <span class="who"><b>{p.name}</b><span class="tag">#{p.tag}</span> <small>waiting</small></span>
              <button class="no" title="cancel request" aria-label="cancel request to {nameTag(p)}" onclick={() => friends.remove(p)}>×</button>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  {/if}
</aside>

<style>
  .fv { height: 100%; display: flex; flex-direction: column; background: rgba(var(--panel-rgb), 0.92); border: 1px solid var(--line-soft); }
  header { display: flex; align-items: center; gap: 8px; height: 22px; border-bottom: 1px solid var(--line-soft); flex: none; }
  .label { height: 100%; display: flex; align-items: center; padding: 0 8px; background: var(--accent); color: var(--ink); font-weight: 700; letter-spacing: 0.04em; }
  .sub { color: var(--dim); }
  .grow { flex: 1; }
  .conn { width: 7px; height: 7px; border-radius: 50%; background: var(--ended); }
  .conn.on { background: var(--green); box-shadow: 0 0 6px var(--green); }
  .ic { all: unset; padding: 0 8px; color: var(--dim); cursor: pointer; }
  .ic:hover, .ic:focus-visible { color: var(--hi); }
  .body { flex: 1; overflow: auto; padding: 8px 10px 14px; }
  .empty { padding: 14px; color: var(--dim); }
  .cta { all: unset; margin-top: 8px; padding: 4px 14px; background: var(--accent); color: var(--ink); font-weight: 700; cursor: pointer; }

  .me { display: flex; align-items: center; gap: 8px; margin-bottom: 8px; }
  .me b { color: var(--hi); font-size: 1.05em; }
  .tag { color: var(--dim); font-weight: 400; }
  .seg { display: flex; }
  .seg button { all: unset; padding: 0 7px; border: 1px solid var(--line); color: var(--dim); cursor: pointer; font-size: 0.88em; }
  .seg button + button { border-left: none; }
  .seg button.on.online { background: var(--green); border-color: var(--green); color: var(--ink); }
  .seg button.on.away { background: var(--amber); border-color: var(--amber); color: var(--ink); }
  .seg button.on.invisible { background: var(--ended); border-color: var(--ended); color: var(--hi); }
  .seg button:focus-visible { outline: 1px solid var(--hi); }

  .add { display: flex; gap: 6px; }
  .add input { all: unset; flex: 1; min-width: 0; padding: 4px 8px; border: 1px solid var(--line); background: rgba(var(--bg-rgb), 0.6); color: var(--hi); }
  .add input:focus { border-color: var(--accent); }
  .add input::placeholder { color: var(--faint); }
  .add button { all: unset; padding: 0 10px; background: var(--accent); color: var(--ink); font-weight: 700; cursor: pointer; }
  .add button[disabled] { opacity: 0.4; cursor: default; }
  .err { color: var(--red); margin: 6px 0 0; }
  .ok { color: var(--green); margin: 6px 0 0; }

  h3 { display: flex; align-items: center; gap: 6px; margin: 14px 0 4px; color: var(--dim); font-size: 0.85em; font-weight: 600; text-transform: uppercase; letter-spacing: 0.05em; }
  .count { padding: 0 6px; border-radius: 8px; background: var(--accent); color: var(--ink); font-weight: 700; }
  ul { list-style: none; margin: 0; padding: 0; }
  .note { color: var(--faint); }
  .note b { color: var(--txt2); }

  .req { display: flex; align-items: center; gap: 6px; padding: 4px 6px; border: 1px solid var(--line-soft); margin-bottom: 4px; }
  .req .who { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--txt); }
  .req small { color: var(--faint); }
  .yes { all: unset; padding: 0 8px; background: var(--green); color: var(--ink); font-weight: 700; cursor: pointer; }
  .no { all: unset; padding: 0 6px; color: var(--dim); cursor: pointer; font-size: 1.2em; }
  .no:hover, .yes:hover { filter: brightness(1.15); color: var(--hi); }

  .friend { margin-bottom: 3px; }
  .row { all: unset; box-sizing: border-box; width: 100%; display: flex; align-items: center; gap: 8px; padding: 5px 6px; border: 1px solid transparent; cursor: pointer; }
  .row:hover, .row:focus-visible { border-color: rgba(var(--accent-rgb), 0.5); background: rgba(var(--accent-rgb), 0.07); }
  .off .row { opacity: 0.6; }
  .fire .row { border-color: rgba(var(--amber-rgb), 0.6); box-shadow: inset 0 -6px 10px -6px rgba(var(--amber-rgb), 0.7); }
  .dot { flex: none; width: 8px; height: 8px; border-radius: 50%; background: var(--ended); }
  .dot.online { background: var(--green); box-shadow: 0 0 6px var(--green); }
  .dot.away { background: var(--amber); }
  .who { flex: 1; min-width: 0; display: grid; }
  .line { display: flex; align-items: baseline; gap: 4px; white-space: nowrap; overflow: hidden; }
  .line b { color: var(--hi); }
  .lv { margin-left: 4px; padding: 0 4px; border: 1px solid rgba(var(--accent-rgb), 0.5); color: var(--accent); font-size: 0.8em; }
  .who small { color: var(--dim); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .spark { flex: none; width: 42px; height: 18px; display: flex; align-items: flex-end; gap: 1px; }
  .spark i { flex: 1; background: var(--txt3); }
  .unread { flex: none; min-width: 16px; padding: 0 4px; box-sizing: border-box; text-align: center; background: var(--warn); color: var(--ink); font-weight: 700; border-radius: 8px; }
</style>
