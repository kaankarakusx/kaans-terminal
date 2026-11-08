<script lang="ts">
  import { fileIcon } from './fileIcon'

  let { path }: { path: string } = $props()
  const icon = $derived(fileIcon(path))
</script>

<span class="fi" style:color={icon.color} aria-hidden="true">{icon.glyph}</span>

<style>
  .fi {
    flex: none;
    display: inline-grid;
    place-items: center;
    width: 1.7em;
    font-size: 0.78em;
    font-weight: 800;
    letter-spacing: -0.04em;
    line-height: 1;
  }
</style>
