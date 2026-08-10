<script lang="ts">
  import { onMount } from 'svelte'
  import { sessions, openTab, closeTab, newSession } from '../stores/sessions.svelte'
  import { chat, STATUS_META } from '../stores/chat.svelte'
  import { toggleSidebar } from '../stores/ui.svelte'
  // Brand mark (single source in src/lib/logo.svg) — inlined raw so the
  // theme tokens in the SVG resolve against the active family×mode.
  import logo from '$lib/logo.svg?raw'

  // Design tab bar (`Pragma UI Kit.dc.html`): logo block + session tabs with
  // status dots, overflow beyond maxVisible (5 wide / 3 narrow / 1 tight,
  // active tab kept in view) collapses into a "+N" menu listing hidden tabs.
  let winWidth = $state(typeof window !== 'undefined' ? window.innerWidth : 1200)
  onMount(() => {
    const onResize = () => (winWidth = window.innerWidth)
    window.addEventListener('resize', onResize)
    return () => window.removeEventListener('resize', onResize)
  })
  const maxVisible = $derived(winWidth < 640 ? 1 : winWidth < 900 ? 3 : 5)

  // Keep the active tab centered in the visible window; push the rest into the
  // overflow menu. No windowing when everything fits.
  const tabWindow = $derived.by(() => {
    const ids = sessions.openTabs
    if (ids.length <= maxVisible) return { visible: ids, hidden: [] as string[] }
    const activeIdx = Math.max(0, ids.indexOf(sessions.activeTabId ?? ''))
    const start = Math.min(
      Math.max(0, activeIdx - Math.floor((maxVisible - 1) / 2)),
      ids.length - maxVisible
    )
    return {
      visible: ids.slice(start, start + maxVisible),
      hidden: ids.filter((id) => !ids.slice(start, start + maxVisible).includes(id)),
    }
  })

  let menuOpen = $state(false)
  let el = $state<HTMLElement | null>(null)

  // Tabs overflow into a hidden-scrollbar strip; route the wheel (and
  // trackpad vertical gesture) over the strip into horizontal scrolling
  // so extra sessions stay reachable without a visible scrollbar.
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

  function pickHidden(id: string) {
    openTab(id)
    menuOpen = false
  }
</script>

<svelte:window
  onclick={(e) => {
    if (menuOpen && el && !e.composedPath().includes(el)) menuOpen = false
  }}
/>

<div class="tab-bar" role="tablist" aria-label="Sessions">
  <!-- Design logo block: P-mark SVG + PRAGMA wordmark. -->
  <span class="logo" aria-label="Pragma">
    {@html logo}
    <span class="wordmark">PRAGMA</span>
  </span>

  <div class="tabs" use:tabsScroll>
    {#each tabWindow.visible as id (id)}
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
          ><span class="msr" aria-hidden="true">close</span></span>
        </button>
      {/if}
    {/each}
  </div>

  {#if tabWindow.hidden.length > 0}
    <div class="more-wrap" bind:this={el}>
      <button
        class="more"
        aria-haspopup="menu"
        aria-expanded={menuOpen}
        onclick={() => (menuOpen = !menuOpen)}
        title="More sessions"
      >+{tabWindow.hidden.length}</button>
      {#if menuOpen}
        <div class="more-menu" role="menu" aria-label="More sessions">
          {#each tabWindow.hidden as id (id)}
            {@const s = sessions.list.find((s) => s.id === id)}
            {@const st = chat.status[id] ?? 'idle'}
            {@const meta = STATUS_META[st]}
            {#if s}
              <button class="m-row" role="menuitem" onclick={() => pickHidden(id)}>
                <span class="m-dot" class:live={st !== 'idle'} style={`background:${meta.color}`} aria-hidden="true"></span>
                <span class="m-name">{s.name}</span>
                <span class="m-status">{meta.label}</span>
              </button>
            {/if}
          {/each}
        </div>
      {/if}
    </div>
  {/if}

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
  /* Design logo block: P-mark + wordmark, right-separated. */
  .logo {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 0 14px 0 7.5px;
    border-right: 1px solid var(--border-default);
    flex-shrink: 0;
  }
  .logo :global(svg) {
    flex: none;
    overflow: visible;
  }
  .wordmark {
    font-family: var(--font-heading);
    font-weight: 600;
    font-size: 15px;
    letter-spacing: 0.1em;
    color: var(--text-primary);
    white-space: nowrap;
  }
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
    animation: fadeIn 0.3s ease both;
    transition: background 0.25s ease, color 0.25s ease, border-color 0.25s ease;
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
  /* Design "+N" overflow pill + session menu. */
  .more-wrap {
    position: relative;
    flex: none;
    display: flex;
    align-items: stretch;
  }
  .more {
    height: 100%;
    padding: 0 11px;
    background: none;
    border: none;
    border-right: 1px solid var(--border-default);
    color: var(--text-muted);
    cursor: pointer;
    font-family: var(--font-mono);
    font-size: 11.5px;
    white-space: nowrap;
    transition: color 0.2s ease, background 0.2s ease;
  }
  .more:hover {
    color: var(--text-primary);
    background: var(--surface-hover);
  }
  .more-menu {
    position: absolute;
    top: calc(100% + 4px);
    right: 0;
    width: 230px;
    max-height: 300px;
    overflow-y: auto;
    background: var(--bg-elevated);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-lg);
    padding: 5px;
    z-index: 40;
    animation: dropIn 0.24s cubic-bezier(0.22, 0.8, 0.3, 1) both;
  }
  .m-row {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 8px;
    background: none;
    border: none;
    border-radius: var(--radius-sm);
    cursor: pointer;
    text-align: left;
    color: var(--text-primary);
    transition: background 0.2s ease;
  }
  .m-row:hover {
    background: var(--surface-hover);
  }
  .m-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    flex: none;
  }
  .m-dot.live {
    animation: cursorblink 1.2s ease-in-out infinite;
  }
  .m-name {
    flex: 1;
    min-width: 0;
    font-family: var(--font-body);
    font-size: 12.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .m-status {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-muted);
  }
  .new-session {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    width: 34px;
    flex: none;
    align-self: stretch;
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

  /* ── Narrow windows ─────────────────────────────────────────────────── */
  @media (max-width: 1040px) {
    .wordmark {
      display: none; /* the PRAGMA wordmark eats width the tabs need */
    }
  }
  @media (max-width: 860px) {
    .tab-name {
      max-width: 96px;
    }
  }
</style>