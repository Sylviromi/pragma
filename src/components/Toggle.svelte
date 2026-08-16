<script lang="ts">
  // Design toggle: 32×18 square-radius track, square knob. Accent fill when on.
  let {
    checked = $bindable(false),
    onChange,
    label,
    disabled = false,
  }: {
    checked?: boolean
    onChange?: (v: boolean) => void
    label?: string
    disabled?: boolean
  } = $props()

  function toggle() {
    if (disabled) return
    checked = !checked
    onChange?.(checked)
  }
</script>

<button
  class="toggle"
  class:on={checked}
  aria-checked={checked}
  aria-label={label}
  title={label}
  role="switch"
  onclick={toggle}
  disabled={disabled}
>
  <span class="knob" aria-hidden="true"></span>
</button>

<style>
  .toggle {
    position: relative;
    width: 32px;
    height: 18px;
    border-radius: 2px;
    border: none;
    cursor: pointer;
    padding: 0;
    background: var(--border-default);
    flex-shrink: 0;
    transition: background 0.28s cubic-bezier(0.22, 0.8, 0.3, 1);
  }
  .toggle:hover {
    border-color: var(--text-muted);
  }
  .toggle.on {
    background: var(--accent);
  }
  .toggle:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 14px;
    height: 14px;
    border-radius: 1px;
    background: var(--bg-elevated);
    display: block;
    transition: left 0.28s cubic-bezier(0.22, 0.8, 0.3, 1), background 0.28s ease;
  }
  .toggle.on .knob {
    left: calc(100% - 16px);
    background: var(--accent-on);
  }
</style>