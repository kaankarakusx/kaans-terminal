<script lang="ts">
  import { confirm } from './dialog.svelte'

  let ok = $state<HTMLButtonElement>()
  $effect(() => {
    if (confirm.open) queueMicrotask(() => ok?.focus())
  })
</script>

<svelte:window
  onkeydown={(e) => {
    if (!confirm.open) return
    if (e.key === 'Escape') confirm.cancel()
  }}
/>

{#if confirm.open}
  <div class="modal" role="alertdialog" aria-modal="true" aria-labelledby="confirm-title">
    <div class="box" class:danger={confirm.open.danger}>
      <b id="confirm-title">{confirm.open.title}</b>
      <p>{confirm.open.body}</p>
      <div class="actions">
        <button onclick={() => confirm.cancel()}>cancel</button>
        <button bind:this={ok} class="go" onclick={() => confirm.accept()}>{confirm.open.action}</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .modal {
    position: fixed;
    inset: 0;
    z-index: 120;
    display: grid;
    place-content: center;
    background: rgba(0, 0, 0, 0.5);
  }
  .box {
    width: min(440px, 90vw);
    display: grid;
    gap: 8px;
    padding: 14px;
    background: var(--panel);
    border: 1px solid var(--line);
    color: var(--txt2);
    backdrop-filter: blur(14px);
  }
  .box.danger { border-color: var(--warn); }
  b { color: var(--hi); }
  p { margin: 0; }
  .actions { display: flex; justify-content: flex-end; gap: 8px; }
  button { all: unset; padding: 2px 10px; border: 1px solid var(--line); cursor: pointer; color: var(--txt2); }
  button:hover, button:focus-visible { border-color: var(--accent); color: var(--hi); }
  .go { background: var(--accent); color: var(--ink); border-color: var(--accent); }
  .danger .go { background: var(--warn); border-color: var(--warn); }
</style>
