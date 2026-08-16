<script lang="ts">
  // Shared markdown changelog styling (audit E4) — rendered release notes,
  // previously duplicated between UpdateDialog and the General settings page.
  import { renderMarkdown } from '../lib/markdown'

  let {
    body,
    empty = 'This version’s release notes weren’t published.',
  }: {
    body: string
    empty?: string
  } = $props()
</script>

{#if body}
  <div class="changelog">
    {@html renderMarkdown(body)}
  </div>
{:else}
  <p class="empty">{empty}</p>
{/if}

<style>
  /* Readable, comfortably sized changelog. renderMarkdown output is safe HTML. */
  .changelog :global(h1), .changelog :global(h2), .changelog :global(h3) {
    margin: 14px 0 6px;
    font-size: 15px;
    font-weight: 600;
  }
  .changelog :global(p) { margin: 6px 0; }
  .changelog :global(ul), .changelog :global(ol) { margin: 6px 0; padding-left: 20px; }
  .changelog :global(li) { margin: 4px 0; }
  .changelog :global(code) {
    font-family: var(--font-mono);
    background: oklch(0 0 0 / 0.25);
    padding: 1px 4px;
    border-radius: 4px;
    font-size: 0.92em;
  }
  .changelog :global(pre) {
    overflow: auto;
    background: oklch(0 0 0 / 0.28);
    padding: 10px;
    border-radius: 8px;
  }
  .empty { color: var(--text-muted); }
</style>
