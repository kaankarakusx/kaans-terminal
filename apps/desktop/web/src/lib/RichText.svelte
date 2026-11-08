<!-- Bağlantıları tıklanabilir metin: http(s) ve @ad (GitHub). Tarayıcıda açılır; HTML yorumlanmaz. -->
<script lang="ts">
  import { openUrl } from '@tauri-apps/plugin-opener'
  import { inTauri } from './engine.svelte'
  import { richText } from './richText'

  let { text }: { text: string } = $props()
  const segs = $derived(richText(text))

  function open(e: MouseEvent, href: string) {
    e.preventDefault()
    if (inTauri) void openUrl(href)
    else window.open(href, '_blank', 'noopener')
  }
</script>

{#each segs as s, i (i)}{#if s.href}<a href={s.href} onclick={(e) => open(e, s.href!)}>{s.text}</a>{:else}{s.text}{/if}{/each}

<style>
  a { color: var(--cyan); text-decoration: underline; text-decoration-color: rgba(var(--cyan-rgb), 0.4); }
  a:hover { text-decoration-color: var(--cyan); }
</style>
