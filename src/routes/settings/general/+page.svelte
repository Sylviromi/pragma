<script lang="ts">
  let enterSend = $state(true)
  let restoreTabs = $state(true)

  $effect(() => {
    enterSend = localStorage.getItem('pragma:enterSend') !== 'false'
    restoreTabs = localStorage.getItem('pragma:restoreTabs') !== 'false'
  })

  function toggle(key: string, value: boolean) {
    localStorage.setItem(`pragma:${key}`, String(value))
  }
</script>

<h2 style="margin-top:0;color:var(--color-text-primary)">General</h2>

<div class="rows">
  <div class="row">
    <div>
      <div class="label">Send on Enter</div>
      <div class="desc">Shift+Enter always inserts a newline</div>
    </div>
    <button
      class="toggle"
      class:on={enterSend}
      aria-pressed={enterSend}
      onclick={() => { enterSend = !enterSend; toggle('enterSend', enterSend) }}
    >
      <span class="knob"></span>
    </button>
  </div>

  <div class="row">
    <div>
      <div class="label">Restore tabs on launch</div>
      <div class="desc">Reopen sessions from your last window</div>
    </div>
    <button
      class="toggle"
      class:on={restoreTabs}
      aria-pressed={restoreTabs}
      onclick={() => { restoreTabs = !restoreTabs; toggle('restoreTabs', restoreTabs) }}
    >
      <span class="knob"></span>
    </button>
  </div>
</div>

<style>
  h2 { font-size: var(--font-size-lg); font-weight: 600; margin-bottom: var(--spacing-4); }
  .rows { display: flex; flex-direction: column; }
  .row {
    display: flex; align-items: center; justify-content: space-between;
    gap: var(--spacing-6); padding: var(--spacing-3) 0;
    border-bottom: 1px solid var(--color-border-subtle);
  }
  .label { font-size: var(--font-size-md); color: var(--color-text-primary); }
  .desc { font-size: var(--font-size-sm); color: var(--color-text-muted); margin-top: 2px; }
  .toggle {
    width: 34px; height: 19px; border-radius: 2px; border: none;
    background: var(--color-border-default); position: relative; cursor: pointer; flex-shrink: 0;
  }
  .toggle.on { background: var(--color-accent-brand); }
  .knob {
    position: absolute; top: 2px; left: 2px;
    width: 15px; height: 15px; border-radius: 1px;
    background: var(--color-bg-elevated); display: block;
    transition: left 0.1s;
  }
  .toggle.on .knob { left: calc(100% - 17px); }
</style>
