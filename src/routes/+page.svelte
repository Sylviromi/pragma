<script lang="ts">
  import { onMount } from 'svelte'
  import ChatPanel from '../components/ChatPanel.svelte'
  import DirBar from '../components/DirBar.svelte'
  import SidebarPanel from '../components/SidebarPanel.svelte'
  import SlotStack from '../components/SlotStack.svelte'
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
      <DirBar sessionId={sessions.activeTabId} />
      <ChatPanel sessionId={sessions.activeTabId} />
    </div>
    <!-- Sidebar stays mounted so the design's open/close animation plays
         (width 0.34s + inner translateX slide); overlay mode on narrow windows. -->
    <div class="sidebar-wrap" class:closed={!ui.sidebarOpen} class:overlay={narrow}>
      <div class="sidebar-inner">
        <SidebarPanel sessionId={sessions.activeTabId} />
      </div>
    </div>
    <!-- Plugin miniscreen slot: docked overlay panels (frontend-dev's live
         preview). Components position themselves (fixed overlay) and follow
         the agent's servers — the app just mounts them; no context needed. -->
    <SlotStack slot="miniscreen" />
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
  /* Design sidebar: 300px (270px when overlaying a narrow window), slides in
     with a width + inner-translate transition; closed collapses to 0. */
  .sidebar-wrap {
    display: flex;
    flex: none;
    width: 300px;
    overflow: hidden;
    opacity: 1;
    transition: width 0.34s cubic-bezier(0.22, 0.8, 0.3, 1), opacity 0.26s ease;
  }
  .sidebar-inner {
    display: flex;
    flex: none;
    width: 300px;
    height: 100%;
    transition: transform 0.34s cubic-bezier(0.22, 0.8, 0.3, 1);
  }
  .sidebar-wrap.closed {
    width: 0;
    opacity: 0;
    pointer-events: none;
  }
  .sidebar-wrap.closed .sidebar-inner {
    transform: translateX(18px);
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
  .sidebar-wrap.overlay,
  .sidebar-wrap.overlay .sidebar-inner {
    width: 270px;
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