<script lang="ts">
  // Search / filter input — icon + borderless field (audit C4). Previously
  // duplicated in the ModelPicker dropdown, plugin/marketplace search bars
  // and the directory picker. `variant` covers the bordered box (settings
  // pages) vs. the underline field (dropdowns, pickers); `mono` switches to
  // the mono typeface (registry + dir picker).
  import Icon from './Icon.svelte'
  import type { Snippet } from 'svelte'

  let {
    value = $bindable(''),
    placeholder = '',
    label,
    mono = false,
    variant = 'box',
    icon = 'search',
    onkeydown,
    trailing,
    inputEl = $bindable(null),
    class: klass = '',
  }: {
    value?: string
    placeholder?: string
    label?: string
    mono?: boolean
    variant?: 'box' | 'underline'
    icon?: string
    onkeydown?: (e: KeyboardEvent) => void
    /** Extra content at the row's end (e.g. a result count). */
    trailing?: Snippet
    /** Bound to the inner <input> element (dropdowns focus it on open). */
    inputEl?: HTMLInputElement | null
    class?: string
  } = $props()
</script>

<div class={`search ${variant}${mono ? ' mono' : ''}${klass ? ' ' + klass : ''}`}>
  <span class="s-ico" aria-hidden="true"><Icon name={icon as never} size={16} /></span>
  <input
    type="search"
    {placeholder}
    bind:value
    bind:this={inputEl}
    aria-label={label}
    {onkeydown}
  />
  {@render trailing?.()}
</div>

<style>
  .search {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
    min-width: 0;
  }
  .s-ico {
    flex: none;
    display: flex;
    color: var(--text-muted);
  }
  input {
    flex: 1;
    min-width: 0;
    background: transparent;
    border: 0;
    outline: none;
    color: var(--text-primary);
    font-family: var(--font-body);
    font-size: 13px;
  }
  input::placeholder {
    color: var(--text-muted);
  }
  .mono input {
    font-family: var(--font-mono);
    font-size: 12.5px;
  }

  /* Bordered box (settings pages / registry). */
  .search.box {
    border: 1px solid var(--border-default);
    background: var(--bg-base);
    padding: 7px 11px;
  }
  /* Underline strip (dropdowns / pickers). */
  .search.underline {
    border-bottom: 1px solid var(--border-default);
    padding: 6px 8px;
  }
</style>
