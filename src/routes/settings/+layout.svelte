<script lang="ts">
  import { page } from '$app/stores'
  let { children } = $props()
  const nav = [
    { label: 'General', href: '/settings/general' },
    { label: 'Providers', href: '/settings/providers' },
    { label: 'Plugins', href: '/settings/plugins' },
    { label: 'Appearance', href: '/settings/appearance' }
  ]
</script>

<div class="shell">
  <nav aria-label="Settings navigation">
    <div class="label">SETTINGS</div>
    {#each nav as item}
      <a
        href={item.href}
        class="item"
        class:active={$page.url.pathname === item.href}
        aria-current={$page.url.pathname === item.href ? 'page' : undefined}
      >{item.label}</a>
    {/each}
  </nav>
  <div class="content">{@render children()}</div>
</div>

<style>
  .shell {
    display: flex;
    height: 100%;
    flex: 1;
    overflow: hidden;
  }
  nav {
    width: 180px;
    background: var(--color-bg-elevated);
    border-right: 1px solid var(--color-border-subtle);
    padding: var(--spacing-3) var(--spacing-2);
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex-shrink: 0;
  }
  .label {
    font-size: var(--font-size-xs);
    font-weight: 700;
    letter-spacing: 0.05em;
    color: var(--color-text-muted);
    padding: var(--spacing-1) var(--spacing-2);
    margin-bottom: var(--spacing-1);
  }
  .item {
    display: block;
    padding: 5px var(--spacing-2);
    border-radius: var(--radius-sm);
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
    text-decoration: none;
    cursor: pointer;
  }
  .item:hover {
    color: var(--color-text-primary);
    background: var(--color-bg-overlay);
  }
  .item.active {
    color: var(--color-text-accent);
    background: color-mix(in srgb, var(--color-accent-brand) 15%, transparent);
  }
  .content {
    flex: 1;
    padding: var(--spacing-6);
    overflow-y: auto;
  }
</style>