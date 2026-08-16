<script lang="ts">
  // Centered "what's new" dialog, shown once per version when an update is
  // available. Readable changelog + Skip (persists per-version; Settings →
  // Update still shows the update) + Install & restart.
  import { updater, installUpdate, dismissDialog } from '../stores/updater.svelte';
  import Modal from './Modal.svelte';
  import Button from './Button.svelte';
  import Changelog from './Changelog.svelte';
  import CornerMarks from './CornerMarks.svelte';
</script>

{#if updater.showDialog && updater.available}
  {@const avail = updater.available}
  <Modal
    onclose={dismissDialog}
    label="Pragma update available"
    padded={false}
    zIndex={20000}
    footClass="upd-actions"
  >
    <!-- blueprint corner marks: accent, per design updDialogStyle -->
    <CornerMarks color="var(--accent)" length={7} />

    {#snippet header()}
      <div class="head">
        <div class="titles">
          <div class="eyebrow">New version available</div>
          <h2 id="upd-title">Pragma {avail.version}</h2>
        </div>
        <button
          class="x"
          onclick={dismissDialog}
          aria-label="Skip this version"
          title="Skip this version">×</button
        >
      </div>
    {/snippet}

    <div class="changelog-scroll">
      <Changelog body={avail.body ?? ''} />
    </div>

    {#if updater.failed}
      <p class="err">Update failed — check your connection and try again.</p>
    {/if}

    {#snippet footer()}
      <Button
        variant="ghost"
        flat
        class="upd-btn"
        onclick={dismissDialog}
        disabled={updater.installing}>Skip this version</Button
      >
      <Button
        variant="primary"
        class="upd-btn"
        onclick={installUpdate}
        disabled={updater.installing}
      >
        {updater.installing ? 'Downloading…' : 'Install & restart'}
      </Button>
    {/snippet}
  </Modal>
{/if}

<style>
  /* Modal card is position:relative (CornerMarks overlays it). */
  .head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
    padding: 18px 20px 14px;
    border-bottom: 1px solid var(--border-subtle);
    flex: none;
  }
  .eyebrow {
    font-size: var(--font-size-xs);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--accent);
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
    color: var(--text-muted);
    font-size: 22px;
    line-height: 1;
    cursor: pointer;
    padding: 0 2px;
  }
  .x:hover {
    color: var(--text-primary);
  }

  /* Scrollable middle section between the fixed head and footer. */
  .changelog-scroll {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 8px 20px 14px;
    font-size: var(--font-size-md);
    line-height: 1.6;
    color: var(--text-primary);
  }

  .err {
    margin: 0;
    padding: 8px 20px;
    color: var(--danger);
    font-size: var(--font-size-sm);
    border-top: 1px solid var(--border-subtle);
    flex: none;
  }

  /* Footer actions row (class applied to Modal's foot wrapper). */
  :global(.upd-actions) {
    padding: 14px 20px 18px;
    border-top: 1px solid var(--border-subtle);
  }
  /* Restore the dialog's original buttons (14px, 8px radius; Skip borderless). */
  :global(.btn.upd-btn) {
    border-radius: 8px;
    padding: 8px 14px;
    font-size: 14px;
    font-weight: 600;
  }
</style>
