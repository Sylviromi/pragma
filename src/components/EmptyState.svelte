<script lang="ts">
  // Empty / missing state — centered muted block with optional icon (audit F).
  // Previously duplicated across the plugins page, marketplace, plugin detail
  // and the session rail.
  import Icon from './Icon.svelte'
  import type { Snippet } from 'svelte'

  let {
    icon,
    children,
    class: klass = '',
  }: {
    icon?: string
    children?: Snippet
    class?: string
  } = $props()
</script>

<div class={`empty-state${klass ? ' ' + klass : ''}`}>
  {#if icon}
    <span class="es-ico" aria-hidden="true"><Icon name={icon as never} size={28} /></span>
  {/if}
  {@render children?.()}
</div>

<style>
  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 44px 0;
    color: var(--text-muted);
    font-size: 12.5px;
    text-align: center;
  }
  .es-ico {
    display: flex;
    color: var(--text-muted);
  }
  .empty-state :global(p) {
    margin: 0;
  }
  .empty-state :global(code) {
    font-family: var(--font-mono);
    font-size: 11.5px;
    color: var(--text-secondary);
  }
  .empty-state :global(a) {
    color: var(--accent);
    font-size: 12px;
  }
</style>
