<script lang="ts">
  // Section header — icon + mono uppercase title + right-aligned note (audit
  // C3). Previously duplicated across the plugins install panel, marketplace
  // registry and plugin-detail section cards.
  import Icon from './Icon.svelte'

  let {
    icon,
    title,
    note = '',
  }: {
    icon?: string
    title: string
    note?: string
  } = $props()
</script>

<div class="sec-head">
  {#if icon}
    <span class="sec-ico" aria-hidden="true"><Icon name={icon as never} size={15} /></span>
  {/if}
  <span class="sec-title">{title}</span>
  {#if note}<span class="sec-note">{note}</span>{/if}
</div>

<style>
  .sec-head {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 9px 14px;
    border-bottom: 1px solid var(--border-subtle);
    background: var(--bg-overlay);
  }
  .sec-ico {
    flex: none;
    display: flex;
    color: var(--accent);
  }
  .sec-title {
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-primary);
  }
  .sec-note {
    margin-left: auto;
    font-family: var(--font-body);
    font-size: 11.5px;
    color: var(--text-muted);
  }
</style>
