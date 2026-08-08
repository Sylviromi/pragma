<script lang="ts">
  import SlotStack from './SlotStack.svelte'
  let { sessionId }: { sessionId: string } = $props()
</script>

<div class="status-bar" data-session-id={sessionId}>
  <SlotStack slot="chat-toolbar-extra" context={sessionId} />
</div>

<style>
  .status-bar:empty {
    display: none;
  }
  .status-bar {
    border-bottom: 1px solid var(--color-border-subtle);
    font-size: var(--font-size-sm);
    font-family: var(--font-mono);
  }
</style>