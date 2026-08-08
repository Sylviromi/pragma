<script lang="ts">
  import { onMount } from 'svelte'
  import ChatPanel from '../components/ChatPanel.svelte'
  import { loadSessions, sessions, openTab } from '../stores/sessions.svelte'

  onMount(async () => {
    await loadSessions()
    const last = localStorage.getItem('pragma:lastTabId')
    if (last && sessions.list.find((s) => s.id === last)) openTab(last)
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