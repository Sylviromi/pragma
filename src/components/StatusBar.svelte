<script lang="ts">
  import SlotStack from './SlotStack.svelte'
  import { pluginsForSlot } from '../stores/plugins.svelte'

  // Render only when a plugin actually claims the `status-bar` slot — the
  // design keeps this strip (handsfree progress, etc.) hidden otherwise.
  const apps = $derived(pluginsForSlot('status-bar'))
</script>

{#if apps.length > 0}
  <div class="status-bar" role="status">
    <SlotStack slot="status-bar" />
  </div>
{/if}

<style>
  /* Design: plugin status bar — sits between the tab bar and the conversation,
     visually distinct (tint, smaller mono text) from the chat below. */
  .status-bar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 16px;
    background: var(--bg-overlay);
    border-bottom: 1px solid var(--border-default);
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-secondary);
    flex: none;
    min-height: 28px;
  }
</style>