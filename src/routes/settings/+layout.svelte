<script lang="ts">
  import { page } from '$app/stores'
  import { goto } from '$app/navigation'
  import { plugins } from '../../stores/plugins.svelte'
  import { updater } from '../../stores/updater.svelte'
  import { providerState, loadProviders } from '../../stores/providers.svelte'
  import { sessions, openTab, loadSessions, restoreTabs } from '../../stores/sessions.svelte'
  import { chat, STATUS_META, type AgentStatus } from '../../stores/chat.svelte'
  import logo from '$lib/logo.svg?raw'
  let { children } = $props()

  // Deep-linked settings (e.g. an opened route) may arrive before the app view
  // ever mounted — load + restore sessions so the chips and back targets work.
  import { onMount } from 'svelte'
  onMount(async () => {
    if (sessions.list.length === 0) {
      await loadSessions()
      restoreTabs()
    }
    // Providers drive the nav count badge — load them lazily (ModelPicker
    // only loads them when the composer mounts).
    if (providerState.providers.length === 0) loadProviders().catch(() => {})
  })

  // Design settings nav (v2 `settingsNav`): icon rows with neutral mono count
  // badges — Providers = provider count, Plugins = plugin count, General = '1'
  // while an app update is available.
  // Design settings nav: grouped — CONFIGURATION (core pages) and EXTENSIONS
  // (marketplace + plugins, sitting next to each other).
  const navGroups = [
    {
      label: 'CONFIGURATION',
      items: [
        { label: 'General', href: '/settings/general', icon: 'tune' },
        { label: 'Providers', href: '/settings/providers', icon: 'key', badge: () => String(providerState.providers.length) },
        { label: 'Verification', href: '/settings/verification', icon: 'fact_check' },
        { label: 'Shortcuts', href: '/settings/shortcuts', icon: 'keyboard' },
        { label: 'Appearance', href: '/settings/appearance', icon: 'palette' },
      ],
    },
    {
      label: 'EXTENSIONS',
      items: [
        { label: 'Marketplace', href: '/settings/marketplace', icon: 'storefront' },
        { label: 'Plugins', href: '/settings/plugins', icon: 'extension', badge: () => String(plugins.list.length) },
      ],
    },
  ]
  const isActive = (href: string) =>
    $page.url.pathname === href || ($page.url.pathname.startsWith('/settings/plugin/') && href === '/settings/plugins')

  // General nav badge: '1' while a Pragma update is available (design).
  const generalBadge = $derived(updater.available ? '1' : '')

  // Session chips: up to 4 open sessions, dot + name; click returns to it.
  function returnToSession(id: string) {
    openTab(id)
    goto('/')
  }
  const sessionChips = $derived(
    sessions.openTabs.slice(0, 4).map((id) => {
      const s = sessions.list.find((x) => x.id === id)
      const status = (chat.status[id] ?? 'idle') as AgentStatus
      const meta = STATUS_META[status]
      return {
        id,
        name: s?.name ?? id,
        live: status !== 'idle',
        dot: meta.color,
        title: `${s?.name ?? id} — ${meta.label}`,
      }
    })
  )

  // Build info pinned at the nav bottom (design `buildInfo`) — version +
  // runtime only; the config path was dropped (couldn't fit the 212px column).
  const buildInfo = $derived([
    { k: 'version', v: updater.currentVersion || 'dev' },
    { k: 'runtime', v: 'tauri' },
  ])
</script>

<div class="shell">
  <!-- Design v2 top bar: logo block + Settings + session chips + back. -->
  <header>
    <span class="logo" aria-label="Pragma">
      {@html logo}
      <span class="wordmark">PRAGMA</span>
    </span>
    <div class="settings-tag">
      <span class="msr" aria-hidden="true">tune</span>
      <span class="settings-label">Settings</span>
    </div>
    <div class="chips">
      {#if sessionChips.length > 0}
        <span class="return-label">Return to</span>
        {#each sessionChips as c (c.id)}
          <button class="chip" class:active={c.id === sessions.activeTabId} title={c.title} onclick={() => returnToSession(c.id)}>
            <span class="chip-dot" class:live={c.live} style={`background:${c.dot}`} aria-hidden="true"></span>
            <span class="chip-name">{c.name}</span>
          </button>
        {/each}
      {/if}
    </div>
    <a class="back" href="/" aria-label="Back to session">
      <span class="msr" aria-hidden="true">arrow_back</span>Session
    </a>
  </header>

  <div class="body">
    <nav aria-label="Settings navigation">
      {#each navGroups as group, gi (group.label)}
        <div class="label" class:alt={gi > 0}>{group.label}</div>
        {#each group.items as item, i (item.href)}
          <a
            href={item.href}
            class="item stagger"
            style={`--i:${i}`}
            class:active={isActive(item.href)}
            aria-current={isActive(item.href) ? 'page' : undefined}
          >
            <span class="item-ico" aria-hidden="true"><span class="msr">{item.icon}</span></span>
            <span class="item-label">{item.label}</span>
            {#if item.label === 'General'}
              {#if generalBadge}<span class="nav-badge">{generalBadge}</span>{/if}
            {:else if item.badge && item.badge() !== ''}
              <span class="nav-badge">{item.badge()}</span>
            {/if}
          </a>
        {/each}
      {/each}

      <div class="flex-spacer"></div>
      <div class="build-info">
        {#each buildInfo as row (row.k)}
          <div class="bi-row">
            <span class="bi-k">{row.k}</span>
            <span class="bi-v">{row.v}</span>
          </div>
        {/each}
      </div>
    </nav>
    <div class="content">{@render children()}</div>
  </div>
</div>

<style>
  .shell {
    display: flex;
    flex-direction: column;
    height: 100%;
    flex: 1;
    overflow: hidden;
  }
  /* Design v2 top bar: 42px elevated strip. */
  header {
    height: 42px;
    display: flex;
    align-items: stretch;
    border-bottom: 1px solid var(--border-default);
    background: var(--bg-elevated);
    flex: none;
    min-width: 0;
  }
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
  .settings-tag {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 12px;
    border-right: 1px solid var(--border-default);
    flex: none;
  }
  .settings-tag .msr {
    font-family: 'Material Symbols Rounded';
    font-size: 16px;
    line-height: 1;
    color: var(--accent);
  }
  .settings-label {
    font-family: var(--font-mono);
    font-size: 10.5px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-primary);
  }
  .chips {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: 1;
    min-width: 0;
    padding: 0 12px;
    overflow: hidden;
  }
  .return-label {
    font-family: var(--font-mono);
    font-size: 9.5px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--text-muted);
    flex: none;
  }
  .chip {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: 0 1 auto;
    min-width: 74px;
    max-width: 170px;
    height: 26px;
    padding: 0 10px;
    background: transparent;
    border: 1px solid var(--border-default);
    color: var(--text-secondary);
    font-family: var(--font-body);
    font-size: 12.5px;
    cursor: pointer;
    transition: border-color 0.18s ease, color 0.18s ease;
  }
  .chip:hover {
    border-color: var(--accent);
    color: var(--text-primary);
  }
  .chip.active {
    background: var(--accent-soft);
    border-color: var(--accent);
  }
  .chip-dot {
    width: 6px;
    height: 6px;
    flex: none;
    border-radius: 50%;
  }
  .chip-dot.live {
    animation: cursorblink 1.2s ease-in-out infinite;
  }
  .chip-name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .back {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 30px;
    padding: 0 11px;
    margin: 6px 10px;
    background: none;
    border: 1px solid var(--border-default);
    color: var(--text-secondary);
    text-decoration: none;
    cursor: pointer;
    font-family: var(--font-mono);
    font-size: 10.5px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    flex: none;
    align-self: center;
    transition: color 0.18s ease, border-color 0.18s ease;
  }
  .back:hover {
    color: var(--text-primary);
    border-color: var(--accent);
  }
  .back .msr {
    font-size: 16px;
    line-height: 1;
  }
  .body {
    display: flex;
    flex: 1;
    min-height: 0;
  }
  /* Design nav: 212px elevated, CONFIGURATION label, icon rows. */
  nav {
    width: 212px;
    flex-shrink: 0;
    background: var(--bg-elevated);
    border-right: 1px solid var(--border-default);
    padding: 14px 10px;
    display: flex;
    flex-direction: column;
    gap: 1px;
    overflow-y: auto;
  }
  .label {
    font-family: var(--font-mono);
    font-size: 11px;
    letter-spacing: 0.12em;
    color: var(--text-muted);
    padding: 0 8px 9px;
  }
  /* second nav group gets breathing room above it */
  .label.alt {
    margin-top: 16px;
  }
  .stagger {
    animation: rowIn 0.34s cubic-bezier(0.22, 0.8, 0.3, 1) both;
    animation-delay: calc(var(--i) * 45ms);
  }
  .flex-spacer {
    flex: 1;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 9px;
    width: 100%;
    text-align: left;
    padding: 8px 10px;
    margin-bottom: 1px;
    background: none;
    border: none;
    border-left: 2px solid transparent;
    font-family: var(--font-body);
    font-size: 13px;
    color: var(--text-secondary);
    text-decoration: none;
    cursor: pointer;
    transition: background 0.2s ease, color 0.2s ease, border-color 0.2s ease;
  }
  .item:hover {
    color: var(--text-primary);
    background: var(--surface-hover);
  }
  .item.active {
    color: var(--text-primary);
    background: var(--accent-soft);
    border-left-color: var(--accent);
  }
  .item-ico {
    flex: none;
    display: flex;
    color: var(--text-muted);
  }
  .item-ico .msr {
    font-size: 17px;
    line-height: 1;
  }
  .item.active .item-ico {
    color: var(--accent);
  }
  .item-label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  /* Design nav badge: neutral mono count pill (settingsNav badgeStyle). */
  .nav-badge {
    flex: none;
    font-family: var(--font-mono);
    font-size: 9.5px;
    color: var(--text-muted);
    background: var(--bg-overlay);
    padding: 1px 6px;
  }
  .build-info {
    border-top: 1px solid var(--border-subtle);
    padding: 11px 8px 2px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .bi-row {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-muted);
  }
  .bi-k {
    flex: none;
  }
  .bi-v {
    color: var(--text-secondary);
    min-width: 0;
  }
  .content {
    flex: 1;
    padding: 24px 30px 40px;
    overflow-y: auto;
    min-width: 0;
    max-width: 1080px;
  }
  @media (max-width: 820px) {
    .content {
      padding: 16px;
    }
    .wordmark {
      display: none;
    }
    .return-label {
      display: none;
    }
  }
</style>
