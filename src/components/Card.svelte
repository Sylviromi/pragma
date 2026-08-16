<script lang="ts">
  // Settings card — elevated bordered card (audit C1). Previously duplicated
  // across general / providers / shortcuts / verification.
  import type { Snippet } from 'svelte'

  let {
    children,
    class: klass = '',
    style,
  }: {
    children?: Snippet
    class?: string
    style?: string
  } = $props()
</script>

<div class={`card${klass ? ' ' + klass : ''}`} {style}>
  {@render children?.()}
</div>

<style>
  .card {
    background: var(--bg-elevated);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-md);
    overflow: hidden;
    max-width: 640px;
  }
</style>
