<script lang="ts">
  import { sessions, openTab, closeTab, newSession } from '../stores/sessions.svelte'
  import { chat } from '../stores/chat.svelte'
  import Icon from './Icon.svelte'

  const autoApprove = $derived(chat.autoApprove[sessions.activeTabId ?? ''] ?? false)

  function toggleAuto() {
    const id = sessions.activeTabId
    if (!id) return
    chat.autoApprove[id] = !(chat.autoApprove[id] ?? false)
  }
</script>

<div class="tab-bar" role="tablist" aria-label="Sessions">
  <span class="logo" aria-label="Pragma">PRAGMA</span>
  <div class="tabs">
    {#each sessions.openTabs as id (id)}
      {@const s = sessions.list.find((s) => s.id === id)}
      {@const running = chat.running[id] ?? false}
      {@const hasError = !!chat.error[id]}
      {#if s}
        <button
          class="tab"
          class:active={sessions.activeTabId === id}
          role="tab"
          aria-selected={sessions.activeTabId === id}
          title={running ? `${s.name} — model active` : hasError ? `${s.name} — last run failed` : `${s.name} — idle`}
          onclick={() => openTab(id)}
        >
          <span class="tab-state" class:running={running} class:error={hasError} aria-hidden="true"></span>
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
          ><Icon name="close" size={13} /></span>
        </button>
      {/if}
    {/each}
  </div>
  <button
    class="new-session"
    onclick={() => newSession()}
    aria-label="New session"
    title="New session"
  >
    <Icon name="add" size={14} />
    <span>New</span>
  </button>
  <div class="actions">
    {#if sessions.activeTabId}
      <button
        class="auto-pill"
        class:on={autoApprove}
        onclick={toggleAuto}
        title="Auto-approve all tool calls without confirmation (active session)"
        aria-pressed={autoApprove}
      >
        <span class="track" aria-hidden="true"><span class="knob"></span></span>
        <span>Auto</span>
      </button>
    {/if}
    <a href="/settings/providers" aria-label="Settings" title="Settings">
      <Icon name="settings" size={20} />
    </a>
    <button aria-label="Plugins" title="Plugins">
      <Icon name="extension" size={20} />
    </button>
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
    border-right: 1px solid var(--color-border-default);
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
    border-right: 1px solid var(--color-border-default);
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
    white-space: nowrap;
    transition: padding 0.12s;
  }
  /* When the close X is present (hover or active) shrink the right padding
     so the X hugs the edge; otherwise the right gap mirrors the left. */
  .tab:hover,
  .tab.active {
    padding-right: var(--spacing-1);
  }
  .tab.active {
    border-right-color: transparent;
    background: var(--color-bg-base);
    color: var(--color-text-primary);
  }
  .tab.active + .tab {
    border-left: 1px solid var(--color-border-subtle);
  }
  /* Model-state dot: pulsing while a run is active, visible-but-dull when
     idle, red on error. Sized so it reads on the active tab's darker bg.
     The right margin mirrors the tab's left padding so the dot breathes. */
  .tab-state {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--color-text-secondary, #9aa5b1);
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--color-text-secondary, #9aa5b1) 30%, transparent);
    flex-shrink: 0;
    margin-right: var(--spacing-1);
  }
  .tab-state.running {
    background: var(--color-accent-brand);
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--color-accent-brand) 38%, transparent);
    animation: tab-pulse 1.5s ease-in-out infinite;
  }
  .tab-state.error {
    background: #f85149;
    box-shadow: 0 0 0 2px color-mix(in srgb, #f85149 38%, transparent);
  }
  @keyframes tab-pulse {
    0%, 100% { box-shadow: 0 0 0 0 color-mix(in srgb, var(--color-accent-brand) 55%, transparent); }
    50% { box-shadow: 0 0 0 5px color-mix(in srgb, var(--color-accent-brand) 0%, transparent); }
  }
  .tab-close {
    display: none;
    width: 18px;
    height: 18px;
    flex-shrink: 0;
    border-radius: var(--radius-sm);
    align-items: center;
    justify-content: center;
  }
  .tab:hover .tab-close,
  .tab.active .tab-close {
    display: flex;
  }
  .tab-close:hover {
    background: var(--color-bg-overlay);
  }
  .new-session {
    align-self: center;
    margin: 0 2px 0 0;
    padding: 3px 10px;
    display: flex;
    align-items: center;
    gap: 5px;
    border-radius: var(--radius-md);
    border: 1px solid var(--color-border-default);
    background: transparent;
    color: var(--color-text-primary);
    font-size: var(--font-size-xs);
    font-weight: 600;
    letter-spacing: 0.02em;
    cursor: pointer;
    transition: border-color 0.12s, background 0.12s, color 0.12s;
  }
  .new-session:hover {
    border-color: var(--color-accent-brand);
    color: var(--color-accent-brand);
    background: color-mix(in srgb, var(--color-accent-brand) 8%, transparent);
  }
  .new-session:focus-visible {
    outline: 2px solid var(--color-border-focus);
    outline-offset: 1px;
  }
  .actions {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 0 var(--spacing-2) 0 0;
  }
  .actions a,
  .actions button {
    background: transparent;
    border: none;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
    padding: var(--spacing-1);
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
  .auto-pill {
    gap: 6px;
    font-size: var(--font-size-xs);
    font-weight: 600;
    letter-spacing: 0.02em;
  }
  .track {
    position: relative;
    width: 26px;
    height: 15px;
    border-radius: 999px;
    background: var(--color-border-default);
    transition: background 0.12s;
    flex-shrink: 0;
  }
  .auto-pill.on .track {
    background: var(--color-accent-brand);
  }
  .knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 11px;
    height: 11px;
    border-radius: 50%;
    background: var(--color-bg-elevated);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.35);
    transition: left 0.12s;
  }
  .auto-pill.on .knob {
    left: calc(100% - 13px);
  }
</style>