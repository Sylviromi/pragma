<script lang="ts">
  // Inline alert row — icon + message (audit D3). Previously the error-row and
  // notice-row were hand-rolled inside MessageList.
  import Icon from './Icon.svelte'
  import type { Snippet } from 'svelte'

  let {
    kind = 'error',
    icon,
    children,
  }: {
    kind?: 'error' | 'info'
    icon?: string
    children?: Snippet
  } = $props()

  const ico = $derived(icon ?? (kind === 'error' ? 'error' : 'info'))
</script>

<div class="inline-alert" class:error={kind === 'error'} class:info={kind === 'info'} role={kind === 'error' ? 'alert' : 'status'}>
  <span class="ia-ico" aria-hidden="true"><Icon name={ico as never} size={14} /></span>
  <span class="ia-text">{@render children?.()}</span>
</div>

<style>
  .inline-alert {
    display: flex;
    align-items: flex-start;
    gap: var(--spacing-2);
    padding: 8px 12px;
    border-radius: var(--radius-md);
    font-size: var(--font-size-sm);
  }
  .inline-alert.error {
    background: color-mix(in srgb, var(--danger) 10%, transparent);
    border: 1px solid color-mix(in srgb, var(--danger) 30%, transparent);
    color: var(--danger);
  }
  .inline-alert.info {
    background: color-mix(in srgb, var(--text-muted) 8%, transparent);
    border: 1px solid color-mix(in srgb, var(--text-muted) 25%, transparent);
    color: var(--text-muted);
  }
  .ia-ico {
    flex: none;
    display: flex;
    margin-top: 1px;
  }
  .ia-text {
    white-space: pre-wrap;
    word-break: break-word;
  }
</style>
