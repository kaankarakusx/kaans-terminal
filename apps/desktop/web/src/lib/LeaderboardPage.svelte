<!--
  Sıralama sayfası (OZELLIKLER.md §6.4, TASARIM.md §5.8): solda liste (bu hafta / tüm zamanlar), sağda seçilen
  kişinin "hacker" kartı. Arkadaşlar ve genel sıralama sunucuyla gelir; şimdilik yalnızca sen (yerel önizleme).
  "On fire" satırların alt kenarında alev yanar. Activity bar'daki kupa ya da üst bardaki rütbe açar; Esc kapatır.
-->
<script lang="ts">
  import { rankOf, totalXp, weekOf } from '@kaans/shared/xp'
  import RankBadge from './RankBadge.svelte'
  import { duration } from './format'
  import { friends } from './friends.svelte'
  import { social, type BoardRow } from './social.svelte'
  import { deriveTheme } from '../themes/derive'
  import { themeById } from '../themes/index'
  import { themes } from '../themes/store.svelte'
  import { ui } from './ui.svelte'
  import { xp } from './xp.svelte'

  let tab = $state<'week' | 'all'>('week')
  /** Seçilen satır (ad#tag); boşsa sen */
  let picked = $state<string | null>(null)

  const online = $derived(social.state.registered)
  // Sunucudaki liste (kayıtlıysan); sekme değişince ve sayfa açıkken dakikada bir yenilenir
  $effect(() => {
    if (!online) return
    const scope = tab
    void social.board(scope)
    void social.refresh()
    const t = setInterval(() => void social.board(scope), 60_000)
    return () => clearInterval(t)
  })

  const me = $derived(social.status)
  const s = $derived(me?.summary ?? xp.summary)
  /** Bu hafta kazanılan XP: sunucudan, yoksa yerel hesap */
  const localWeekXp = $derived.by(() => {
    const start = weekOf(xp.today)
    return xp.summary.totalXp - totalXp(xp.days.filter((d) => d.day < start), xp.today).total
  })
  const weekXp = $derived(me?.weekXp ?? localWeekXp)
  const todaySecs = $derived(me?.todaySecs ?? xp.todaySecs)
  const weekSecs = $derived(me?.weekSecs ?? xp.weekSecs)
  const myLast7 = $derived(me ? me.last7.map((x) => Math.floor(x / 60)) : xp.last7.map((d) => d.min))
  const onFire = $derived(me?.onFire ?? xp.onFire)
  const restedLeft = $derived(me?.restedLeftMin ?? xp.restedLeft)
  const n = (v: number) => v.toLocaleString('en-US')

  const localRow: BoardRow = $derived({
    name: social.state.name ?? 'you', tag: social.state.tag ?? '', you: true, level: s.level,
    xp: tab === 'week' ? weekXp : s.totalXp, theme: themes.id, online: true, onFire,
    todaySecs, last7: myLast7.map((m) => m * 60),
  })
  const board = $derived(online ? social.boards[tab] : undefined)
  const rows: BoardRow[] = $derived(board?.rows ?? [localRow])
  const working = $derived(board?.working ?? 1)
  const key = (r: BoardRow) => `${r.name}#${r.tag}`
  const sel = $derived(rows.find((r) => key(r) === picked) ?? rows.find((r) => r.you) ?? localRow)
  const selRank = $derived(sel.you ? s.rank : rankOf(sel.level))
  const selLast7 = $derived(sel.you ? myLast7 : (sel.last7 ?? []).map((x) => Math.floor(x / 60)))
  const selTheme = $derived(sel.you ? themes.shown : sel.theme ? themeById(sel.theme) : null)
  const position = $derived(rows.indexOf(sel) + 1)
  /** Kart o kişinin temasında (OZELLIKLER.md §6.4): temanın token'ları yalnızca kartın içine yazılır */
  const cardStyle = $derived(
    selTheme && !sel.you
      ? Object.entries(deriveTheme(selTheme)).map(([k, v]) => `${k}: ${v}`).join('; ')
      : undefined,
  )
  const maxDay = $derived(Math.max(60, ...selLast7))
  /** Son 7 günün etiketleri (bugün en sağda) */
  const dayLabels = $derived(
    Array.from({ length: 7 }, (_, i) => (i === 6 ? 'today' : new Date(Date.now() - (6 - i) * 86_400_000).toLocaleDateString('en-US', { weekday: 'short', day: 'numeric' }))),
  )

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && !e.defaultPrevented && !social.picking) ui.board = false
  }
</script>

<svelte:window {onkeydown} />

<div class="page">
  <section class="box board" aria-label="leaderboard">
    <header>
      <span class="label">leaderboard</span>
      <span class="grow"></span>
      <span class="meta">everyone with XP</span>
      <button class="close" title="close (Esc)" aria-label="close leaderboard" onclick={() => (ui.board = false)}>×</button>
    </header>
    <div class="bar">
      <span class="seg" role="tablist">
        <button role="tab" class:on={tab === 'week'} aria-selected={tab === 'week'} onclick={() => (tab = 'week')}>THIS WEEK</button>
        <button role="tab" class:on={tab === 'all'} aria-selected={tab === 'all'} onclick={() => (tab = 'all')}>ALL TIME</button>
      </span>
      <span class="hint"><i class="live"></i> {working} working now · {tab === 'week' ? 'XP earned this week' : 'total XP'} · click someone for their week</span>
    </div>
    <ol class="rows">
      {#each rows as r, i (key(r))}
        <li class="row" class:you={r.you} class:fire={r.onFire} class:sel={key(r) === key(sel)}>
          <button class="hit" aria-label="show {r.name}'s week" onclick={() => (picked = key(r))}></button>
          <span class="pos">{i + 1}</span>
          <span class="who">
            <span class="name">
              {#if r.online}<i class="live" title="working now"></i>{/if}
              <b>{r.name}</b>{#if r.tag}<span class="tag">#{r.tag}</span>{/if}
              <span class="lv">Lv {r.level}</span>
              {#if r.you}<span class="me">you</span>{/if}
            </span>
            <small>{rankOf(r.level).name}{r.todaySecs != null ? ` · ${duration(r.todaySecs)} today` : ''}</small>
          </span>
          <span class="spark" aria-hidden="true">
            {#each r.last7 ?? [] as m, k (k)}<i style:height="{m ? 12 + (88 * m) / Math.max(3600, ...(r.last7 ?? [])) : 4}%"></i>{/each}
          </span>
          <span class="score">+{n(r.xp)} XP</span>
          <span class="act">
            {#if online && !r.you && r.tag}
              {#if r.friend === 'friends'}
                <button class="mini" title="message {r.name}" aria-label="message {r.name}#{r.tag}" onclick={() => friends.openChat(r)}>✉</button>
              {:else if r.friend === 'sent'}
                <span class="sent">sent</span>
              {:else if r.friend === 'received'}
                <button class="mini add" onclick={async () => { await friends.accept(r); void social.board(tab) }}>accept</button>
              {:else}
                <button class="mini add" onclick={async () => { await friends.add(`${r.name}#${r.tag}`); void social.board(tab) }}>+ add</button>
              {/if}
            {/if}
          </span>
          {#if r.onFire}<span class="flames" aria-hidden="true"></span>{/if}
        </li>
      {/each}
    </ol>
    {#if !online}
      <div class="soon">
        <p>You're not on the leaderboard yet. Pick a <b>hacker name</b> to join, add friends by <b>name#tag</b> and see who's working now.</p>
        <button class="cta" onclick={() => (social.picking = true)}>choose a hacker name</button>
      </div>
    {:else if social.offline}
      <p class="soon">Can't reach the server right now; showing the last list.</p>
    {/if}
  </section>

  <section class="box hacker" aria-label="{sel.name}'s card" style={cardStyle}>
    <header>
      <span class="label">hacker</span>
      <span class="grow"></span>
      <span class="meta">in their theme · {selTheme?.name ?? 'unknown'}</span>
    </header>
    <div class="body">
      <div class="who">
        <RankBadge rank={selRank} size={64} glow />
        <div>
          <small>#{position} on the leaderboard{sel.you ? ' · you' : ''}</small>
          <b class="big">{sel.name}{#if sel.tag}<span class="tag">#{sel.tag}</span>{/if}</b>
          <div class="lvline" style:color={selRank.color}>Level {sel.level} · {selRank.name}</div>
        </div>
      </div>

      {#if sel.you}
        <div class="xpbar" style:--rc={s.rank.color}><i style:width="{s.nextXp ? Math.min(100, (s.levelXp / s.nextXp) * 100) : 100}%"></i></div>
        <div class="xpline">
          <span>{s.nextXp ? `${n(s.levelXp)} / ${n(s.nextXp)} XP to level ${s.level + 1}` : 'max level'}</span>
          {#if restedLeft > 0}<span class="rested">Rested: {duration(restedLeft * 60)} left at 2×</span>{/if}
        </div>
      {/if}

      <div class="tiles">
        <div class="tile"><small>TODAY</small><b>{sel.todaySecs != null ? duration(sel.todaySecs) : 'hidden'}</b>{#if sel.onFire}<em>🔥 on fire</em>{/if}</div>
        <div class="tile"><small>{tab === 'week' ? 'THIS WEEK' : 'ALL TIME'}</small><b>+{n(sel.xp)} XP</b></div>
        {#if sel.you}
          <div class="tile"><small>ACTIVE</small><b>{duration(weekSecs)}</b><em>this week</em></div>
          <div class="tile"><small>TOTAL</small><b>{n(s.totalXp)} XP</b></div>
          <div class="tile"><small>STREAK</small><b>{s.streakWeeks} wk</b>{#if s.multiplier > 1}<em>+{Math.round((s.multiplier - 1) * 100)}% XP</em>{/if}</div>
        {/if}
      </div>

      {#if selLast7.length}
        <h3>LAST 7 DAYS</h3>
        <div class="days">
          {#each selLast7 as m, i (i)}
            <div class="day">
              <small class="t">{m ? duration(m * 60) : '—'}</small>
              <span class="col"><i style:height="{m ? 6 + (94 * m) / maxDay : 2}%" style:--rc={selRank.color}></i></span>
              <small class="d" class:today={i === 6}>{dayLabels[i]}</small>
            </div>
          {/each}
        </div>
      {:else}
        <p class="note">{sel.name} keeps their time private.</p>
      {/if}

      {#if selTheme}
        <div class="theme">
          <small>THEME</small>
          <b>{selTheme.name}</b>
          {#if sel.you}<span>yours</span>{:else if selTheme.id === themes.id}<span>same as yours</span>{:else}<button class="use" onclick={() => themes.select(selTheme.id)}>use this theme</button>{/if}
        </div>
      {/if}
      <p class="note">{online ? `Synced with the server as ${social.state.name}#${social.state.tag}.` : 'Local preview: pick a hacker name to sync.'} Active minutes count when you're at the keyboard and a chat or terminal is working.</p>
    </div>
  </section>
</div>

<style>
  .page { height: 100%; background: var(--bg); display: grid; grid-template-columns: minmax(0, 1.4fr) minmax(320px, 1fr); gap: 8px; min-height: 0; }
  .box { display: flex; flex-direction: column; min-height: 0; background: var(--bg); border: 1px solid var(--line-soft); }
  header { flex: none; display: flex; align-items: center; gap: 10px; height: 22px; border-bottom: 1px solid var(--line-soft); }
  .label { height: 100%; display: flex; align-items: center; padding: 0 8px; background: var(--accent); color: var(--ink); font-weight: 700; letter-spacing: 0.04em; }
  .grow { flex: 1; }
  .meta { color: var(--dim); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; padding-right: 8px; }
  .close { all: unset; padding: 0 10px; color: var(--dim); cursor: pointer; }
  .close:hover, .close:focus-visible { color: var(--hi); }

  .bar { display: flex; align-items: center; gap: 14px; padding: 10px 12px; }
  .seg { display: flex; }
  .seg button { all: unset; padding: 3px 12px; border: 1px solid var(--line); color: var(--dim); font-weight: 700; letter-spacing: 0.06em; cursor: pointer; }
  .seg button + button { border-left: none; }
  .seg button.on { background: var(--accent); border-color: var(--accent); color: var(--ink); }
  .seg button:focus-visible { outline: 1px solid var(--hi); }
  .hint { color: var(--dim); }
  .live { display: inline-block; width: 7px; height: 7px; border-radius: 50%; background: var(--green); box-shadow: 0 0 6px var(--green); animation: live 1.6s ease-in-out infinite; }
  @keyframes live { 50% { opacity: 0.35; } }

  .rows { list-style: none; margin: 0; padding: 0 12px; overflow: auto; }
  .hit { all: unset; position: absolute; inset: 0; cursor: pointer; }
  .hit:focus-visible { outline: 1px solid var(--accent); }
  .row.sel { background: rgba(var(--accent-rgb), 0.1); }
  .big .tag { color: var(--dim); font-size: 0.7em; font-weight: 400; }
  .cta { all: unset; margin-top: 8px; padding: 4px 14px; background: var(--accent); color: var(--ink); font-weight: 700; cursor: pointer; }
  .cta:focus-visible { outline: 1px solid var(--hi); }
  .use { all: unset; margin-left: auto; padding: 1px 10px; border: 1px solid var(--accent); color: var(--accent); cursor: pointer; }
  .use:hover, .use:focus-visible { background: var(--accent); color: var(--ink); }
  .act { position: relative; z-index: 1; display: flex; justify-content: flex-end; }
  .mini { all: unset; padding: 0 8px; border: 1px solid var(--line); color: var(--txt2); cursor: pointer; }
  .mini:hover, .mini:focus-visible { border-color: var(--accent); color: var(--hi); }
  .mini.add { border-color: rgba(var(--green-rgb), 0.6); color: var(--green); }
  .sent { color: var(--faint); }
  .row { position: relative; display: grid; grid-template-columns: 2.4ch minmax(0, 1fr) 70px 100px 64px; align-items: center; gap: 12px; padding: 8px 10px; border: 1px solid transparent; margin-bottom: 6px; }
  .row.fire { margin-bottom: 14px; }
  .row.you { border-color: rgba(var(--accent-rgb), 0.7); background: rgba(var(--accent-rgb), 0.07); }
  .pos { color: var(--dim); text-align: right; font-weight: 700; }
  .who { display: grid; gap: 2px; min-width: 0; }
  .name { display: flex; align-items: center; gap: 6px; white-space: nowrap; }
  .name b { color: var(--hi); }
  .tag { color: var(--dim); }
  .lv { padding: 0 5px; border: 1px solid rgba(var(--accent-rgb), 0.5); color: var(--accent); font-size: 0.85em; }
  .me { padding: 0 5px; background: var(--accent); color: var(--ink); font-size: 0.85em; font-weight: 700; }
  .who small { color: var(--dim); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .spark { display: flex; align-items: flex-end; gap: 2px; height: 22px; }
  .spark i { flex: 1; background: var(--txt2); }
  .score { color: var(--hi); font-weight: 700; text-align: right; font-variant-numeric: tabular-nums; }
  /* On fire satırı: alt kenarda yanan alevler */
  .row.fire { border-color: var(--accent); box-shadow: 0 0 14px rgba(var(--accent-rgb), 0.35); }
  .flames {
    position: absolute;
    left: -1px;
    right: -1px;
    bottom: -10px;
    height: 20px;
    pointer-events: none;
    background:
      radial-gradient(7px 15px at 50% 100%, var(--amber), transparent 70%) 0 0 / 16px 100% repeat-x,
      radial-gradient(6px 12px at 50% 100%, var(--red), transparent 70%) 8px 3px / 13px 100% repeat-x,
      radial-gradient(4px 7px at 50% 100%, var(--hi), transparent 70%) 4px 9px / 16px 100% repeat-x;
    filter: blur(0.4px) drop-shadow(0 0 4px rgba(var(--amber-rgb), 0.8));
    animation: burn 0.9s steps(3) infinite alternate;
  }
  @keyframes burn { 50% { transform: scaleY(1.25) translateY(-1px); opacity: 0.85; } }
  .soon { color: var(--faint); margin: 10px 12px; }
  .soon b { color: var(--txt3); font-weight: 600; }

  .hacker .body { flex: 1; overflow: auto; padding: 14px; display: grid; gap: 12px; align-content: start; }
  .hacker .who { display: flex; align-items: center; gap: 14px; }
  .hacker .who small { color: var(--dim); }
  .big { display: block; color: var(--hi); font-size: 1.5em; }
  .lvline { font-weight: 700; }
  .xpbar { height: 6px; background: var(--line-soft); }
  .xpbar i { display: block; height: 100%; background: var(--rc); box-shadow: 0 0 8px var(--rc); transition: width 0.6s; }
  .xpline { display: flex; flex-wrap: wrap; gap: 4px 14px; color: var(--txt2); }
  .rested { color: var(--cyan); }
  .tiles { display: grid; grid-template-columns: repeat(auto-fill, minmax(120px, 1fr)); gap: 8px; }
  .tile { display: grid; gap: 2px; padding: 8px 10px; border: 1px solid var(--line-soft); background: rgba(var(--panel-rgb), 0.6); }
  .tile small { color: var(--dim); letter-spacing: 0.08em; font-size: 0.8em; }
  .tile b { color: var(--hi); font-size: 1.15em; }
  .tile em { font-style: normal; color: var(--dim); font-size: 0.85em; }
  h3 { margin: 4px 0 0; color: var(--accent); font-size: 0.85em; letter-spacing: 0.08em; }
  .days { display: grid; grid-template-columns: repeat(7, 1fr); gap: 6px; height: 120px; border-bottom: 1px solid var(--line-soft); padding-bottom: 4px; }
  .day { display: grid; grid-template-rows: auto 1fr auto; justify-items: center; gap: 4px; min-width: 0; }
  .day .t { color: var(--txt3); font-size: 0.8em; white-space: nowrap; }
  .col { width: 60%; height: 100%; display: flex; align-items: flex-end; }
  .col i { width: 100%; background: linear-gradient(var(--rc), color-mix(in oklch, var(--rc) 55%, transparent)); box-shadow: 0 0 8px color-mix(in oklch, var(--rc) 50%, transparent); }
  .day .d { color: var(--dim); font-size: 0.8em; white-space: nowrap; }
  .day .d.today { color: var(--hi); font-weight: 700; }
  .theme { display: flex; align-items: baseline; gap: 10px; padding: 10px 12px; border: 1px solid rgba(var(--accent-rgb), 0.4); background: rgba(var(--accent-rgb), 0.06); }
  .theme small { color: var(--dim); letter-spacing: 0.08em; font-size: 0.8em; }
  .theme b { color: var(--accent); }
  .theme span { margin-left: auto; color: var(--dim); }
  .note { color: var(--faint); margin: 0; font-size: 0.85em; }
  @media (max-width: 1100px) { .page { grid-template-columns: 1fr; } }
  @media (prefers-reduced-motion: reduce) { .flames, .live { animation: none; } }
</style>
