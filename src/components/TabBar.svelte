<script lang="ts">
  import { sessions, openTab, closeTab, newSession } from '../stores/sessions.svelte'
  import { chat, STATUS_META } from '../stores/chat.svelte'
  import { toggleSidebar } from '../stores/ui.svelte'
  import Icon from './Icon.svelte'

  const autoApprove = $derived(chat.autoApprove[sessions.activeTabId ?? ''] ?? false)

  function toggleAuto() {
    const id = sessions.activeTabId
    if (!id) return
    chat.autoApprove[id] = !(chat.autoApprove[id] ?? false)
  }

  /* Tabs overflow into a hidden-scrollbar strip; route the wheel (and
     trackpad vertical gesture) over the strip into horizontal scrolling
     so extra sessions stay reachable without a visible scrollbar. */
  function tabsScroll(el: HTMLElement) {
    const onWheel = (e: WheelEvent) => {
      if (el.scrollWidth <= el.clientWidth) return
      const dx = e.deltaX !== 0 ? e.deltaX : e.deltaY
      el.scrollLeft += dx
      e.preventDefault()
    }
    el.addEventListener('wheel', onWheel, { passive: false })
    return {
      destroy: () => el.removeEventListener('wheel', onWheel)
    }
  }
</script>

<div class="tab-bar" role="tablist" aria-label="Sessions">
  <span class="logo" aria-label="Pragma">PRAGMA</span>
  <div class="tabs" use:tabsScroll>
    {#each sessions.openTabs as id (id)}
      {@const s = sessions.list.find((s) => s.id === id)}
      {@const st = chat.status[id] ?? 'idle'}
      {@const meta = STATUS_META[st]}
      {#if s}
        <button
          class="tab"
          class:active={sessions.activeTabId === id}
          role="tab"
          aria-selected={sessions.activeTabId === id}
          title={`${s.name} — ${meta.label}`}
          onclick={() => openTab(id)}
        >
          <span
            class="tab-state"
            class:live={st !== 'idle'}
            style={`background:${meta.color}`}
            aria-hidden="true"
          ></span>
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
          ><span class="msr" aria-hidden="true">close</span></span
          >
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
    <span class="msr" aria-hidden="true">add</span>
    <span class="new-label">New</span>
  </button>
  <div class="actions">
    {#if sessions.activeTabId}
      <button
        class="auto-toggle"
        class:on={autoApprove}
        onclick={toggleAuto}
        title="Auto-approve all tool calls without confirmation (active session)"
        aria-pressed={autoApprove}
      >
        <span class="track" aria-hidden="true"><span class="knob"></span></span>
        <span class="auto-label">Auto</span>
      </button>
    {/if}
    <a href="/settings/appearance" aria-label="Settings" title="Settings">
      <span class="msr" aria-hidden="true">settings</span>
    </a>
    <button aria-label="Toggle sidebar" title="Toggle sidebar" onclick={toggleSidebar}>
      <span class="msr" aria-hidden="true">right_panel_open</span>
    </button>
    <a href="/settings/plugins" aria-label="Plugins" title="Plugins">
      <span class="msr" aria-hidden="true">extension</span>
    </a>
  </div>
</div>

<style>
  /* Design: 42px browser-style tab bar on --bg-elevated. */
  .tab-bar {
    display: flex;
    align-items: stretch;
    background: var(--bg-elevated);
    border-bottom: 1px solid var(--border-default);
    height: 42px;
    user-select: none;
    flex-shrink: 0;
  }
  .logo {
    padding: 0 14px;
    display: flex;
    align-items: center;
    font-family: var(--font-heading);
    font-weight: 600;
    font-size: 15px;
    letter-spacing: 0.02em;
    color: var(--text-primary);
    border-right: 1px solid var(--border-default);
    flex-shrink: 0;
  }
  /* Fade-out at the right edge (see original impl note) */
  .tabs {
    display: flex;
    overflow-x: auto;
    scrollbar-width: none;
    flex: 1;
    -webkit-mask-image: linear-gradient(
      to right,
      #000 0,
      #000 calc(100% - 64px),
      rgba(0, 0, 0, 0.35) calc(100% - 24px),
      transparent 100%
    );
    mask-image: linear-gradient(
      to right,
      #000 0,
      #000 calc(100% - 64px),
      rgba(0, 0, 0, 0.35) calc(100% - 24px),
      transparent 100%
    );
  }
  .tabs::-webkit-scrollbar {
    display: none;
  }
  .tab {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 14px;
    background: transparent;
    border: none;
    border-top: 2px solid transparent;
    border-right: 1px solid var(--border-default);
    color: var(--text-muted);
    font-size: 13px;
    white-space: nowrap;
    flex-shrink: 0;
    min-width: 0;
    max-width: 180px;
    cursor: pointer;
  }
  .tab:hover,
  .tab.active {
    padding-right: 8px;
  }
  .tab.active {
    background: var(--bg-base);
    border-top-color: var(--accent);
    color: var(--text-primary);
  }
  .tab-state {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    flex-shrink: 0;
    margin-right: 2px;
  }
  .tab-state.live {
    animation: cursorblink 1.2s ease-in-out infinite;
  }
  .tab-name {
    flex: 1;
    min-width: 0;
    font-family: var(--font-body);
    font-size: 13px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .tab-close {
    display: none;
    width: 18px;
    height: 18px;
    flex-shrink: 0;
    border-radius: 2px;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
  }
  .tab:hover .tab-close,
  .tab.active .tab-close {
    display: flex;
  }
  .tab-close:hover {
    background: var(--surface-hover);
    color: var(--text-primary);
  }
  .new-session {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    width: 34px;
    flex: none;
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
  }
  .new-session:hover {
    color: var(--text-primary);
    background: var(--surface-hover);
  }
  .new-label {
    display: none;
  }
  .actions {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 0 10px;
    border-left: 1px solid var(--border-default);
    flex-shrink: 0;
  }
  .actions a,
  .actions button {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    background: none;
    border: none;
    color: var(--text-secondary);
    border-radius: 4px;
    text-decoration: none;
    cursor: pointer;
  }
  .actions a:hover,
  .actions button:hover {
    color: var(--text-primary);
    background: var(--surface-hover);
  }
  .auto-toggle {
    display: flex;
    align-items: center;
    gap: 6px;
    width: auto !important;
    padding: 0 6px;
    font-size: var(--font-size-xs);
    font-weight: 600;
    letter-spacing: 0.02em;
    color: var(--text-secondary);
    background: none;
    border: none;
    cursor: pointer;
  }
  .track {
    position: relative;
    width: 26px;
    height: 15px;
    border-radius: 2px;
    background: var(--border-default);
    transition: background 0.12s;
    flex-shrink: 0;
  }
  .auto-toggle.on .track {
    background: var(--accent);
  }
  .knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 11px;
    height: 11px;
    border-radius: 1px;
    background: var(--bg-elevated);
    transition: left 0.12s;
  }
  .auto-toggle.on .knob {
    left: calc(100% - 13px);
    background: var(--accent-on);
  }

  /* ── Narrow windows ─────────────────────────────────────────────────── */
  @media (max-width: 1040px) {
    .logo {
      display: none; /* the PRAGMA wordmark eats width the tabs need */
    }
  }
  @media (max-width: 940px) {
    .auto-label {
      display: none;
    }
  }
  @media (max-width: 860px) {
    .tab-name {
      max-width: 96px;
    }
  }
</style>