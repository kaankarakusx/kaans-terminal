<!-- Duyurular (OZELLIKLER.md §6.5): sol sütunda (activity bar'daki zil). Açınca okundu sayılır. -->
<script lang="ts">
  import RichText from './RichText.svelte'
  import { ago } from './format'
  import { news } from './news.svelte'
  import { now } from './now.svelte'
  import { ui } from './ui.svelte'

  // Açılınca liste yenilenir; kapanırken okundu (yeni olanlar bu açılışta vurgulu kalsın)
  const seenAtOpen = news.seen
  $effect(() => {
    void news.load()
    return () => news.markSeen()
  })

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && !e.defaultPrevented) ui.news = false
  }
</script>

<svelte:window {onkeydown} />

<aside class="nv" aria-label="announcements">
  <header>
    <span class="label">announcements</span>
    <span class="grow"></span>
    <button class="ic" title="close (Esc)" aria-label="close" onclick={() => (ui.news = false)}>×</button>
  </header>
  <div class="body">
    {#each news.items as a (a.id)}
      <article class:new={Number(a.id) > seenAtOpen}>
        <div class="head"><b>{a.title}</b><span class="grow"></span><small>{ago(now.value - a.at)} ago</small></div>
        <p><RichText text={a.body} /></p>
      </article>
    {:else}
      <p class="empty">No announcements yet.</p>
    {/each}
  </div>
</aside>

<style>
  .nv { height: 100%; display: flex; flex-direction: column; background: rgba(var(--panel-rgb), 0.92); border: 1px solid var(--line-soft); }
  header { display: flex; align-items: center; gap: 8px; height: 22px; border-bottom: 1px solid var(--line-soft); flex: none; }
  .label { height: 100%; display: flex; align-items: center; padding: 0 8px; background: var(--accent); color: var(--ink); font-weight: 700; letter-spacing: 0.04em; }
  .grow { flex: 1; }
  .ic { all: unset; padding: 0 8px; color: var(--dim); cursor: pointer; }
  .ic:hover, .ic:focus-visible { color: var(--hi); }
  .body { flex: 1; overflow: auto; padding: 8px 10px; display: grid; gap: 8px; align-content: start; }
  article { padding: 8px 10px; border: 1px solid var(--line-soft); border-left: 2px solid var(--line); }
  article.new { border-left-color: var(--accent); background: rgba(var(--accent-rgb), 0.06); }
  .head { display: flex; align-items: baseline; gap: 8px; }
  .head b { color: var(--hi); }
  small { color: var(--faint); white-space: nowrap; }
  p { margin: 6px 0 0; color: var(--txt2); white-space: pre-wrap; word-break: break-word; user-select: text; }
  .empty { color: var(--faint); }
</style>
