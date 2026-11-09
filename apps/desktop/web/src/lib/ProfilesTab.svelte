<!--
  PROFILES: Claude Code'un ayrı hesapları (CLAUDE_CONFIG_DIR klasörleri). Seçilen profilin sohbetleri, maliyeti,
  limitleri ve harcama defteri gösterilir. Profiller kabuk takma adlarından (`alias claude-work='CLAUDE_CONFIG_DIR=…
  claude'`) ve ev klasöründeki `.claude*` klasörlerinden algılanır; hiçbir dosyaya yazılmaz.
-->
<script lang="ts">
  import { onMount } from 'svelte'
  import { profiles, tilde } from './profiles.svelte'

  onMount(() => void profiles.load())
</script>

<h3>claude profile</h3>
<p class="note">
  Each profile is a separate Claude account with its own folder. The chats, costs, limits and spend ledger shown in the
  app follow the profile you pick here. Terminals are not changed: your own commands still open the account they always
  did.
</p>

<div class="list" role="radiogroup" aria-label="claude profile">
  {#each profiles.list as p (p.dir)}
    <label class="opt" class:on={p.active}>
      <input type="radio" name="claude-profile" checked={p.active} disabled={profiles.busy} onchange={() => profiles.select(p)} />
      <span class="name">{p.name}</span>
      <span class="dir">{tilde(p.dir)}</span>
      <span class="grow"></span>
      {#each p.commands as c (c)}<code>{c}</code>{/each}
    </label>
  {:else}
    <p class="note">Looking for profiles…</p>
  {/each}
</div>
{#if profiles.error}<p class="err" role="alert">✕ {profiles.error}</p>{/if}

<p class="note">
  Found from shell aliases that set <code>CLAUDE_CONFIG_DIR</code> (for example
  <code>alias claude-work='CLAUDE_CONFIG_DIR="$HOME/.claude-work" claude'</code>) and from <code>~/.claude*</code>
  folders. Limits in the top bar need the limits integration turned on for each profile (INTEGRATIONS, while that
  profile is selected). Token savers (rtk, caveman) stay on the default profile.
</p>

<style>
  h3 { margin: 10px 0 6px; color: var(--dim); font-size: 0.85em; font-weight: 600; text-transform: uppercase; letter-spacing: 0.05em; }
  .list { display: flex; flex-direction: column; gap: 6px; margin: 10px 0; }
  .opt { display: flex; align-items: center; gap: 10px; padding: 6px 10px; border: 1px solid var(--line); color: var(--txt2); cursor: pointer; }
  .opt.on { border-color: var(--accent); color: var(--hi); background: rgba(var(--accent-rgb), 0.08); }
  .opt input { accent-color: var(--accent); margin: 0; }
  .name { font-weight: 700; }
  .dir { color: var(--dim); }
  .grow { flex: 1; }
  code { padding: 0 5px; border: 1px solid var(--line-soft); color: var(--txt3); }
  .note { color: var(--faint); margin: 6px 0; }
  .note code { border: none; padding: 0; color: var(--dim); }
  .err { color: var(--red); }
</style>
