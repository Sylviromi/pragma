<script lang="ts">
  import { sessions, newSession, closeTab } from '../stores/sessions.svelte'
  import { chat, STATUS_META, type AgentStatus } from '../stores/chat.svelte'
  import { toggleRail, toggleSidebar, ui } from '../stores/ui.svelte'
  import { sessionTint } from '../lib/tint'
  // Brand mark (single source in src/lib/logo.svg) — inlined raw so the
  // theme tokens in the SVG resolve against the active family×mode.
  import logo from '$lib/logo.svg?raw'

  // Design tab bar (`Pragma UI Kit v2.dc.html`): slim chrome — logo block,
  // session-rail toggle, the ACTIVE session's dot + name + status, and the
  // icon actions (Settings / Sidebar / Plugins). Sessions themselves live in
  // the left rail (`SessionRail.svelte`), not in browser-style tabs here.

  const active = $derived(
    sessions.activeTabId
      ? sessions.list.find((s) => s.id === sessions.activeTabId) ?? null
      : null
  )
  const activeStatus = $derived(
    ((sessions.activeTabId ? chat.status[sessions.activeTabId] : undefined) ?? 'idle') as AgentStatus
  )
  const statusMeta = $derived(STATUS_META[activeStatus])
  const tint = $derived(sessionTint(sessions.activeTabId))
  const activeDot = $derived(
    activeStatus === 'idle'
      ? tint
      : statusMeta.color
  )

  // Global session shortcuts stay available with the rail UI (Ctrl+T new,
  // Ctrl+W close, Ctrl+B rail, Ctrl+Tab switch).
  $effect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!(e.ctrlKey || e.metaKey)) return
      const k = e.key.toLowerCase()
      if (k === 't') {
        e.preventDefault()
        newSession()
      } else if (k === 'w') {
        e.preventDefault()
        if (sessions.activeTabId) closeTab(sessions.activeTabId)
      } else if (k === 'b') {
        e.preventDefault()
        toggleRail()
      } else if (k === 'tab') {
        e.preventDefault()
        const ids = sessions.openTabs
        if (ids.length < 2 || !sessions.activeTabId) return
        const idx = ids.indexOf(sessions.activeTabId)
        const next = ids[(idx + 1) % ids.length]
        sessions.activeTabId = next
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  })
</script>

<div class="tab-bar" aria-label="App bar">
  <!-- Design logo block: P-mark SVG + PRAGMA wordmark. -->
  <span class="logo" aria-label="Pragma">
    {@html logo}
    <span class="wordmark">PRAGMA</span>
  </span>

  <!-- Session rail toggle (design `railToggleStyle`). -->
  <button
    class="rail-toggle"
    class:open={ui.railOpen}
    onclick={toggleRail}
    aria-label={ui.railOpen ? 'Hide session rail' : 'Show session rail'}
    title={ui.railOpen ? 'Hide session rail' : 'Show session rail'}
    aria-pressed={ui.railOpen}
  >
    <span class="msr" aria-hidden="true">left_panel_open</span>
  </button>

  <!-- Active session: tinted status dot + name + mono status label. -->
  <div class="active" role="status" aria-label={active ? `${active.name} — ${statusMeta.label}` : 'No session'}>
    {#if active}
      <span
        class="active-dot"
        class:live={activeStatus !== 'idle'}
        style={`background:${activeDot}`}
        aria-hidden="true"
      ></span>
      <span class="active-name" title={active.name}>{active.name}</span>
      <span class="active-status">{statusMeta.label}</span>
    {:else}
      <span class="active-name muted">No session</span>
    {/if}
  </div>

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
  /* Design: 42px slim chrome on --bg-elevated. */
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
  /* Rail toggle: 34px, right-separated, tinted when the rail is open. */
  .rail-toggle {
    width: 34px;
    flex: none;
    background: none;
    border: none;
    border-right: 1px solid var(--border-default);
    color: var(--text-muted);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: color 0.2s ease, background 0.2s ease;
  }
  .rail-toggle:hover {
    color: var(--text-primary);
    background: var(--surface-hover);
  }
  .rail-toggle.open {
    color: var(--text-primary);
  }
  .rail-toggle .msr {
    font-size: 18px;
    line-height: 1;
  }
  /* Active session display. */
  .active {
    display: flex;
    align-items: center;
    gap: 9px;
    flex: 1;
    min-width: 0;
    padding: 0 14px;
  }
  .active-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    flex: none;
  }
  .active-dot.live {
    animation: cursorblink 1.2s ease-in-out infinite;
  }
  .active-name {
    font-family: var(--font-body);
    font-size: 13.5px;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }
  .active-name.muted {
    color: var(--text-muted);
  }
  .active-status {
    font-family: var(--font-mono);
    font-size: 10.5px;
    color: var(--text-muted);
    white-space: nowrap;
    flex: none;
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
      display: none; /* the PRAGMA wordmark eats width the session name needs */
    }
  }
</style>
