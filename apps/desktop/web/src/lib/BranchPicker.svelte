<!--
  Dal seçici (VS Code'daki gibi): repo satırındaki ⑂ dal adına tıklayınca açılır. Süz, geç, yeni dal oluştur
  (şu anki daldan ya da seçilen bir daldan), uzak dalı takip eden yerel dal olarak aç. ↑/↓, Enter, Esc.
-->
<script lang="ts">
  import { untrack } from 'svelte'
  import { confirm } from './dialog.svelte'
  import { ago } from './format'
  import { git } from './git.svelte'
  import { now } from './now.svelte'
  import type { GitBranch } from './types'

  let { root, current, onclose }: { root: string; current: string; onclose: () => void } = $props()

  /** pick: dallar + eylemler · base: yeni dalın temeli seçiliyor · name: yeni dalın adı yazılıyor */
  let mode = $state<'pick' | 'base' | 'name'>('pick')
  let base = $state<string | null>(null)
  let query = $state('')
  let sel = $state(0)
  let input = $state<HTMLInputElement>()
  let box = $state<HTMLDivElement>()

  // untrack: loadBranches `git.branches`'ı okuyup yazar; izlenirse efekt kendini sonsuza dek tetikler.
  $effect(() => {
    const r = root
    untrack(() => void git.loadBranches(r))
    queueMicrotask(() => untrack(() => input?.focus()))
  })

  /** VS Code gibi: boşluklar `-` olur */
  const clean = (s: string) => s.trim().replace(/\s+/g, '-')

  interface Item {
    key: string
    label: string
    sub?: string
    when?: number
    current?: boolean
    act: () => void
  }

  const all = $derived(git.branches[root] ?? [])
  const match = (b: GitBranch) => b.name.toLowerCase().includes(query.trim().toLowerCase())

  const items = $derived.by<Item[]>(() => {
    const branchItem = (b: GitBranch, act: () => void): Item => ({
      key: (b.remote ? 'r:' : 'l:') + b.name,
      label: b.name,
      sub: b.subject,
      when: b.time,
      current: b.current,
      act,
    })
    if (mode === 'name') {
      const name = clean(query)
      return name
        ? [{ key: 'create', label: `+ create branch "${name}"`, sub: `from ${base ?? current}`, act: () => void go(name, { create: true, from: base }) }]
        : []
    }
    if (mode === 'base') return all.filter(match).map((b) => branchItem(b, () => pickBase(b.name)))
    const list: Item[] = []
    const typed = clean(query)
    if (typed && !all.some((b) => !b.remote && b.name === typed)) {
      list.push({ key: 'create-typed', label: `+ create branch "${typed}"`, sub: `from ${current}`, act: () => void go(typed, { create: true }) })
    } else {
      list.push({ key: 'create', label: '+ create new branch…', act: () => toName(null) })
    }
    list.push({ key: 'create-from', label: '+ create new branch from…', act: () => toBase() })
    for (const b of all.filter((b) => !b.remote && match(b))) list.push(branchItem(b, () => void go(b.name, {})))
    for (const b of all.filter((b) => b.remote && match(b))) list.push(branchItem(b, () => void go(b.name, {})))
    return list
  })

  /** Uzak dallar başlığı bu öğeden önce */
  const firstRemote = $derived(mode === 'pick' ? items.findIndex((i) => i.key.startsWith('r:')) : -1)

  $effect(() => {
    void items.length
    if (sel >= items.length) sel = Math.max(0, items.length - 1)
  })

  function toName(from: string | null) {
    mode = 'name'
    base = from
    query = ''
    sel = 0
    input?.focus()
  }

  function toBase() {
    mode = 'base'
    query = ''
    sel = 0
    input?.focus()
  }

  function pickBase(name: string) {
    toName(name)
  }

  async function go(name: string, o: { create?: boolean; from?: string | null }) {
    if (!o.create && all.find((b) => b.name === name)?.current) return onclose()
    onclose()
    const err = await git.checkout(root, name, o)
    if (!err) return
    if (err.includes('would be overwritten')) {
      confirm.ask({
        title: `Switch to ${name}?`,
        body: 'Your local changes would be overwritten. Stash them (untracked files too) and switch? The stash stays: restore it later with git stash pop.',
        action: 'stash & switch',
        run: async () => {
          const e2 = await git.checkout(root, name, { ...o, stash: true })
          if (e2) git.errors[root] = e2
        },
      })
    } else if (err !== 'busy') {
      git.errors[root] = err
    }
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault()
      if (mode === 'pick') onclose()
      else {
        mode = 'pick'
        query = ''
        sel = 0
      }
    } else if (e.key === 'ArrowDown') {
      e.preventDefault()
      sel = Math.min(items.length - 1, sel + 1)
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      sel = Math.max(0, sel - 1)
    } else if (e.key === 'Enter') {
      e.preventDefault()
      items[sel]?.act()
    }
  }

  function outside(e: PointerEvent) {
    const t = e.target as Element
    // Açan düğme kendi tıklamasıyla kapatır (yoksa kapanıp hemen yeniden açılır)
    if (box && !box.contains(t) && !t.closest?.('[data-branch-trigger]')) onclose()
  }

  const placeholder = $derived(
    mode === 'name' ? `new branch name (from ${base ?? current})` : mode === 'base' ? 'create from which branch?' : 'switch to branch, or type a new name',
  )
</script>

<svelte:window onpointerdown={outside} />

<div class="picker" bind:this={box} role="dialog" aria-label="branches">
  <input
    bind:this={input}
    bind:value={query}
    {placeholder}
    aria-label={placeholder}
    spellcheck="false"
    autocomplete="off"
    oninput={() => (sel = 0)}
    {onkeydown}
  />
  <ul role="listbox" aria-label="branches">
    {#each items as it, i (it.key)}
      {#if i === firstRemote}<li class="sec" role="presentation">remote</li>{/if}
      <li role="option" aria-selected={i === sel}>
        <button class:sel={i === sel} class:action={!it.key.includes(':')} onpointerenter={() => (sel = i)} onclick={it.act}>
          <span class="mark">{it.current ? '✓' : ''}</span>
          <span class="name">{it.label}</span>
          {#if it.sub}<span class="sub">{it.sub}</span>{/if}
          {#if it.when}<span class="when">{ago(now.value - it.when)}</span>{/if}
        </button>
      </li>
    {:else}
      <li class="none">{mode === 'name' ? 'type a branch name' : git.branches[root] ? 'no matching branch' : 'loading…'}</li>
    {/each}
  </ul>
</div>

<style>
  .picker {
    position: absolute;
    top: calc(100% + 2px);
    left: 6px;
    right: 6px;
    z-index: 3;
    display: flex;
    flex-direction: column;
    /* --panel saydam (glass): arkadaki liste görünmesin diye opak */
    background: rgb(var(--panel-rgb));
    border: 1px solid var(--accent);
    box-shadow: 0 6px 20px rgba(var(--bg-rgb), 0.6);
  }
  input {
    margin: 6px;
    padding: 4px 6px;
    background: rgba(var(--bg-rgb), 0.6);
    color: var(--txt);
    border: 1px solid var(--line);
    font: inherit;
  }
  input:focus { outline: none; border-color: var(--accent); }
  input::placeholder { color: var(--faint); }
  ul { list-style: none; margin: 0; padding: 0 0 4px; max-height: 320px; overflow-y: auto; }
  button {
    all: unset;
    box-sizing: border-box;
    display: flex;
    align-items: baseline;
    gap: 6px;
    width: 100%;
    padding: 3px 10px 3px 4px;
    color: var(--txt2);
    cursor: pointer;
  }
  button.sel { background: rgba(var(--accent-rgb), 0.15); color: var(--hi); }
  button.action { color: var(--accent); }
  button.action.sel { color: var(--hi); }
  .mark { width: 1.2ch; flex: none; color: var(--accent); text-align: center; }
  .name { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; flex: none; max-width: 60%; }
  .sub { color: var(--dim); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; min-width: 0; flex: 1; }
  .when { color: var(--faint); flex: none; font-variant-numeric: tabular-nums; }
  .sec { padding: 6px 10px 2px; color: var(--dim); font-size: 0.85em; font-weight: 600; text-transform: uppercase; letter-spacing: 0.05em; border-top: 1px solid var(--line-soft); margin-top: 4px; }
  .none { padding: 4px 10px; color: var(--faint); }
</style>
