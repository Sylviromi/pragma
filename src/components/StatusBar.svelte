<script lang="ts">
  let { sessionId }: { sessionId: string } = $props()
</script>

<div class="status-bar" id="slot-chat-toolbar-extra" data-session-id={sessionId}></div>

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