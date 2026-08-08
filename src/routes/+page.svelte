<script lang="ts">
  import { onMount } from 'svelte'
  import ChatPanel from '../components/ChatPanel.svelte'
  import { loadSessions, sessions, restoreTabs } from '../stores/sessions.svelte'

  onMount(async () => {
    await loadSessions()
    // Reopen every session tab that was open before the app restarted,
    // including the previously active one.
    restoreTabs()
  })
</script>

{#if sessions.activeTabId}
  <ChatPanel sessionId={sessions.activeTabId} />
{:else}
  <div class="empty">Open or create a session to start.</div>
{/if}

<style>
  .empty {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: var(--color-text-muted);
  }
</style>