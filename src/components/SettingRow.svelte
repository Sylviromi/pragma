<script lang="ts">
  // Settings row — label + desc left, control right (audit C2). Previously
  // duplicated across general / verification / appearance.
  import type { Snippet } from 'svelte'

  let {
    label,
    desc,
    bare = false,
    children,
    class: klass = '',
  }: {
    label: string
    desc?: string
    /** Page-level row (appearance): border-top, no card padding. */
    bare?: boolean
    children?: Snippet
    class?: string
  } = $props()
</script>

<div class={`row${bare ? ' bare' : ''}${klass ? ' ' + klass : ''}`}>
  <div class="row-text">
    <div class="label">{label}</div>
    {#if desc}<div class="desc">{desc}</div>{/if}
  </div>
  {@render children?.()}
</div>

<style>
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 12px 16px;
    border-bottom: 1px solid var(--border-subtle);
  }
  .row:last-child {
    border-bottom: none;
  }
  .row.bare {
    padding: 14px 0;
    border-top: 1px solid var(--border-subtle);
  }
  .row-text {
    min-width: 0;
  }
  .label {
    color: var(--text-primary);
    font-size: 13.5px;
  }
  .desc {
    color: var(--text-muted);
    font-size: 12px;
    margin-top: 1px;
  }
</style>
