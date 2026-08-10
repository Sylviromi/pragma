<script lang="ts">
  import { onMount } from 'svelte'
  import ChatPanel from '../components/ChatPanel.svelte'
  import SidebarPanel from '../components/SidebarPanel.svelte'
  import { loadSessions, sessions, restoreTabs } from '../stores/sessions.svelte'
  import { ui } from '../stores/ui.svelte'

  let narrow = $state(false)

  onMount(async () => {
    await loadSessions()
    // Reopen every session tab that was open before the app restarted,
    // including the previously active one.
    restoreTabs()
  })

  $effect(() => {
    const onResize = () => {
      narrow = window.innerWidth < 900
      // Don't force-close here: the sidebar stays open by default at any
      // window size (on narrow windows it becomes an overlay until the user
      // toggles it).
    }
    onResize()
    window.addEventListener('resize', onResize)
    return () => window.removeEventListener('resize', onResize)
  })
</script>

{#if sessions.activeTabId}
  <div class="app-view">
    <div class="chat-col">
      <ChatPanel sessionId={sessions.activeTabId} />
    </div>
    {#if ui.sidebarOpen}
      <div class="sidebar-wrap" class:overlay={narrow}>
        <SidebarPanel sessionId={sessions.activeTabId} />
      </div>
    {/if}
  </div>
{:else}
  <div class="empty">Open or create a session to start.</div>
{/if}

<style>
  .app-view {
    display: flex;
    flex: 1;
    min-height: 0;
    min-width: 0;
    position: relative;
  }
  .chat-col {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }
  .sidebar-wrap {
    display: flex;
    flex: none;
  }
  /* Narrow: the sidebar docks over the conversation (design: 270px overlay
     with shadow-lg), still driven by the tab-bar toggle. */
  .sidebar-wrap.overlay {
    position: absolute;
    top: 0;
    right: 0;
    bottom: 0;
    z-index: 30;
    box-shadow: var(--shadow-lg);
  }
  .sidebar-wrap.overlay :global(.sidebar) {
    width: 270px;
  }
  .empty {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: var(--color-text-muted);
  }
</style>