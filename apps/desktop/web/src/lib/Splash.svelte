<script lang="ts">
  let { status }: { status: string } = $props()
</script>

<div class="splash">
  <img src="/logo.svg" alt="" width="96" height="96" />
  <div class="logo">
    <small>KAAN'S</small>
    <b>TERMINAL</b>
  </div>
  <div class="status">{status}</div>
</div>

<style>
  .splash {
    height: 100%;
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 18px;
  }
  .logo {
    display: grid;
    justify-items: center;
    line-height: 1.1;
  }
  .logo small {
    color: var(--txt3);
    letter-spacing: 0.3em;
  }
  .logo b {
    font-size: 2.2em;
    letter-spacing: 0.2em;
    color: var(--hi);
    text-shadow: 0 0 12px rgba(var(--accent-rgb), 0.8), 0 0 28px var(--logo);
  }
  .status {
    color: var(--dim);
    animation: blink 1.4s ease-in-out infinite;
  }
  @keyframes blink {
    50% { opacity: 0.35; }
  }
</style>
