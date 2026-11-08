<script lang="ts">
  import { STATUS_COLOR, STATUS_LABEL, type Status } from './types'

  let { status, label = false }: { status: Status; label?: boolean } = $props()
</script>

<span class="st" style:--c={STATUS_COLOR[status]}>
  <i class:pulse={status === 'busy' || status === 'waiting'}></i>
  {#if label}<span>{STATUS_LABEL[status]}</span>{/if}
</span>

<style>
  .st {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    color: var(--c);
    white-space: nowrap;
  }
  i {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--c);
    box-shadow: 0 0 6px var(--c);
    flex: none;
  }
  .pulse { animation: pulse 1.6s ease-in-out infinite; }
  @keyframes pulse { 50% { opacity: 0.35; } }
</style>
