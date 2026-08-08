<script lang="ts">
  import { sessions, openTab, closeTab, newSession } from '../stores/sessions.svelte'
</script>

<div class="tab-bar" role="tablist" aria-label="Sessions">
  <span class="logo" aria-label="Pragma">PRAGMA</span>
  <div class="tabs">
    {#each sessions.openTabs as id (id)}
      {@const s = sessions.list.find((s) => s.id === id)}
      {#if s}
        <button
          class="tab"
          class:active={sessions.activeTabId === id}
          role="tab"
          aria-selected={sessions.activeTabId === id}
          onclick={() => openTab(id)}
        >
          <span class="tab-name">{s.name}</span>
          <span
            class="tab-close"
            role="button"
            tabindex="0"
            aria-label="Close {s.name}"
            onclick={(e) => {
              e.stopPropagation()
              closeTab(id)
            }}
            onkeydown={(e) => e.key === 'Enter' && (e.stopPropagation(), closeTab(id))}
          >×</span>
        </button>
      {/if}
    {/each}
  </div>
  <button class="new-tab" onclick={() => newSession()} aria-label="New session">+</button>
  <div class="actions">
    <a href="/settings/providers" aria-label="Settings">⚙</a>
    <button aria-label="Plugins">⧉</button>
  </div>
</div>

<style>
  .tab-bar {
    display: flex;
    align-items: stretch;
    background: var(--color-bg-elevated);
    border-bottom: 1px solid var(--color-border-subtle);
    min-height: 36px;
    user-select: none;
    flex-shrink: 0;
  }
  .logo {
    padding: 0 var(--spacing-3);
    display: flex;
    align-items: center;
    font-size: var(--font-size-sm);
    font-weight: 700;
    color: var(--color-text-accent);
    border-right: 1px solid var(--color-border-subtle);
    flex-shrink: 0;
    letter-spacing: 0.08em;
  }
  .tabs {
    display: flex;
    overflow-x: auto;
    scrollbar-width: none;
    flex: 1;
  }
  .tabs::-webkit-scrollbar {
    display: none;
  }
  .tab {
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
    padding: 0 var(--spacing-3);
    background: transparent;
    border: none;
    border-top: 2px solid transparent;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
    white-space: nowrap;
  }
  .tab.active {
    border-top-color: var(--color-accent-brand);
    background: var(--color-bg-base);
    color: var(--color-text-primary);
  }
  .tab-close {
    opacity: 0;
    font-size: 14px;
    padding: 2px 3px;
    border-radius: var(--radius-sm);
  }
  .tab:hover .tab-close,
  .tab.active .tab-close {
    opacity: 1;
  }
  .tab-close:hover {
    background: var(--color-bg-overlay);
  }
  .new-tab {
    padding: 0 var(--spacing-3);
    background: transparent;
    border: none;
    color: var(--color-text-muted);
    font-size: 18px;
  }
  .actions {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 0 var(--spacing-3);
  }
  .actions a,
  .actions button {
    background: transparent;
    border: none;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
    padding: var(--spacing-1) var(--spacing-2);
    border-radius: var(--radius-sm);
    text-decoration: none;
    display: flex;
    align-items: center;
  }
  .actions a:hover,
  .actions button:hover {
    color: var(--color-text-primary);
    background: var(--color-bg-overlay);
  }
</style>