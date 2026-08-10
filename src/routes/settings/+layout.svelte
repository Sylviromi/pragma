<script lang="ts">
  import { page } from '$app/stores'
  import { plugins } from '../../stores/plugins.svelte'
  import Icon from '../../components/Icon.svelte'
  let { children } = $props()

  const nav = [
    { label: 'General', href: '/settings/general' },
    { label: 'Providers', href: '/settings/providers' },
    { label: 'Skills', href: '/settings/skills' },
    { label: 'Plugins', href: '/settings/plugins' },
    { label: 'Shortcuts', href: '/settings/shortcuts' },
    { label: 'Appearance', href: '/settings/appearance' }
  ]
  const isActive = (href: string) =>
    $page.url.pathname === href || ($page.url.pathname.startsWith('/settings/plugin/') && href === '/settings/plugins')
</script>

<div class="shell">
  <!-- Top bar per design: back link + "Settings" heading. -->
  <header>
    <a class="back" href="/" aria-label="Back to app">
      <Icon name="chevron_left" size={16} />
    </a>
    <h1>Settings</h1>
  </header>

  <div class="body">
    <nav aria-label="Settings navigation">
      <div class="label">SETTINGS</div>
      {#each nav as item, i}
        <a
          href={item.href}
          class="item stagger"
          style={`--i:${i}`}
          class:active={isActive(item.href)}
          aria-current={isActive(item.href) ? 'page' : undefined}
        >{item.label}</a>
      {/each}

      {#if plugins.list.length > 0}
        <div class="label plugins-label">PLUGINS</div>
        {#each plugins.list as p, i (p.id)}
          <a
            href={`/settings/plugin/${p.id}`}
            class="item plugin-item stagger"
            style={`--i:${i + 6}`}
            class:active={$page.url.pathname === `/settings/plugin/${p.id}`}
            aria-current={$page.url.pathname === `/settings/plugin/${p.id}` ? 'page' : undefined}
          >
            <span class="dot" class:off={!p.enabled} aria-hidden="true"></span>
            {p.name}
          </a>
        {/each}
      {/if}
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
  header {
    height: 42px;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 14px;
    border-bottom: 1px solid var(--border-default);
    background: var(--bg-elevated);
    flex: none;
  }
  .back {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    border-radius: 3px;
    color: var(--text-secondary);
    text-decoration: none;
  }
  .back:hover {
    color: var(--text-primary);
    background: var(--bg-overlay);
  }
  h1 {
    margin: 0;
    font-family: var(--font-heading);
    font-size: 15px;
    font-weight: 600;
    color: var(--text-primary);
  }
  .body {
    display: flex;
    flex: 1;
    min-height: 0;
  }
  nav {
    width: 200px;
    background: var(--bg-elevated);
    border-right: 1px solid var(--border-default);
    padding: 14px 10px 20px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex-shrink: 0;
    overflow-y: auto;
  }
  .label {
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.08em;
    color: var(--text-muted);
    padding: 6px 8px 4px;
  }
  /* Design settings nav entrance: rows fade up 45ms apart. */
  .stagger {
    animation: rowIn 0.34s cubic-bezier(0.22, 0.8, 0.3, 1) both;
    animation-delay: calc(var(--i) * 45ms);
  }
  .plugins-label {
    margin-top: 10px;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 6px 8px;
    border-radius: 3px;
    color: var(--text-muted);
    font-size: 13px;
    text-decoration: none;
    cursor: pointer;
  }
  .item:hover {
    color: var(--text-primary);
    background: var(--surface-hover);
  }
  .item.active {
    color: var(--text-primary);
    background: var(--accent-soft);
  }
  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--success);
    flex: none;
  }
  .dot.off {
    background: var(--text-muted);
  }
  .content {
    flex: 1;
    padding: 24px 28px;
    overflow-y: auto;
    min-width: 0;
  }
  @media (max-width: 820px) {
    .content {
      padding: 16px;
    }
  }
</style>