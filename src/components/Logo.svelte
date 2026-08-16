<script lang="ts">
  // Brand mark (single source in src/lib/logo.svg) — inlined raw so the
  // theme tokens in the SVG resolve against the active family×mode. Used by
  // the main tab bar and the settings shell (was duplicated in both).
  import logo from '$lib/logo.svg?raw'

  let { label = 'Pragma' }: { label?: string } = $props()
</script>

<span class="logo" aria-label={label}>
  {@html logo}
  <span class="wordmark">PRAGMA</span>
</span>

<style>
  .logo {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 0 14px 0 7.5px;
    border-right: 1px solid var(--border-default);
    flex-shrink: 0;
  }
  .logo :global(svg) {
    flex: none;
    overflow: visible;
  }
  .wordmark {
    font-family: var(--font-heading);
    font-weight: 600;
    font-size: 15px;
    letter-spacing: 0.1em;
    color: var(--text-primary);
    white-space: nowrap;
  }
</style>
