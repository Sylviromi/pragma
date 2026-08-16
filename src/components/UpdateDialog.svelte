<script lang="ts">
  // Centered "what's new" dialog, shown once per version when an update is
  // available. Readable changelog + Skip (persists per-version; Settings →
  // Update still shows the update) + Install & restart.
  import { updater, installUpdate, dismissDialog } from '../stores/updater.svelte'
  import { renderMarkdown } from '../lib/markdown'
  import CornerMarks from './CornerMarks.svelte'
</script>

{#if updater.showDialog && updater.available}
  <div
    class="overlay"
    role="presentation"
    tabindex="-1"
    onclick={(e) => {
      if (e.target === e.currentTarget) dismissDialog()
    }}
    onkeydown={(e) => {
      if (e.key === 'Escape') dismissDialog()
    }}
  >
    <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="upd-title">
      <!-- blueprint corner marks: accent, per design updDialogStyle -->
      <CornerMarks color="var(--accent)" length={7} />
      <div class="head">
        <div class="titles">
          <div class="eyebrow">New version available</div>
          <h2 id="upd-title">Pragma {updater.available.version}</h2>
        </div>
        <button class="x" onclick={dismissDialog} aria-label="Skip this version" title="Skip this version">×</button>
      </div>

      <div class="changelog">
        {#if updater.available.body}
          {@html renderMarkdown(updater.available.body)}
        {:else}
          <p class="empty">This version’s release notes weren’t published.</p>
        {/if}
      </div>

      {#if updater.failed}
        <p class="err">Update failed — check your connection and try again.</p>
      {/if}

      <div class="actions">
        <button class="b secondary" onclick={dismissDialog} disabled={updater.installing}>Skip this version</button>
        <button class="b primary" onclick={installUpdate} disabled={updater.installing}>
          {updater.installing ? 'Downloading…' : 'Install & restart'}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 20000;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 20px;
    background: oklch(9% 0.02 250 / 0.55);
    backdrop-filter: blur(2px);
  }
  .dialog {
    position: relative;
    width: min(560px, 94vw);
    max-height: 82vh;
    display: flex;
    flex-direction: column;
    background: var(--color-bg-elevated);
    border: 1px solid var(--color-border-default);
    border-radius: 14px;
    box-shadow: var(--shadow-lg);
    overflow: hidden;
  }
  .head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
    padding: 18px 20px 14px;
    border-bottom: 1px solid var(--color-border-subtle);
  }
  .eyebrow {
    font-size: var(--font-size-xs);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--color-accent-brand);
    margin-bottom: 4px;
  }
  .titles h2 {
    margin: 0;
    font-size: var(--font-size-lg);
    font-weight: 700;
  }
  .x {
    background: none;
    border: none;
    color: var(--color-text-muted);
    font-size: 22px;
    line-height: 1;
    cursor: pointer;
    padding: 0 2px;
  }
  .x:hover { color: var(--color-text-primary); }

  /* Readable, comfortably sized changelog. renderMarkdown output is safe HTML. */
  .changelog {
    flex: 1;
    overflow: auto;
    padding: 8px 20px 14px;
    font-size: var(--font-size-md);
    line-height: 1.6;
    color: var(--color-text-primary);
  }
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
  .empty { color: var(--color-text-muted); }

  .err {
    margin: 0;
    padding: 8px 20px;
    color: var(--color-accent-error);
    font-size: var(--font-size-sm);
    border-top: 1px solid var(--color-border-subtle);
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    padding: 14px 20px 18px;
    border-top: 1px solid var(--color-border-subtle);
  }
  .b {
    border: none;
    border-radius: 8px;
    padding: 8px 14px;
    font-size: var(--font-size-md);
    cursor: pointer;
  }
  .b:disabled { opacity: 0.6; cursor: default; }
  .secondary { background: transparent; color: var(--color-text-secondary); }
  .secondary:hover { background: var(--color-bg-overlay); }
  .primary { background: var(--color-accent-brand); color: var(--color-accent-on); font-weight: 600; }
</style>
