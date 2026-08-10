<script lang="ts">
  // Static keymap — the harness's built-in shortcuts. (Remapping UI + plugin
  // shortcut registration is tracked in TODO.md.)
  const shortcuts = [
    { keys: ['Enter'], action: 'Send message' },
    { keys: ['Shift', 'Enter'], action: 'New line in the composer' },
    { keys: ['Ctrl', 'Shift', 'S'], action: 'Capture app screenshot' },
    { keys: ['Esc'], action: 'Stop the running turn' },
    { keys: ['Ctrl', 'T'], action: 'New session tab' },
    { keys: ['Ctrl', 'W'], action: 'Close active tab' },
    { keys: ['Ctrl', 'Tab'], action: 'Switch session tab' },
    { keys: ['Ctrl', ','], action: 'Open shortcuts' },
    { keys: ['Ctrl', 'K'], action: 'Command palette' },
    { keys: ['Ctrl', '.'], action: 'Toggle skills' }
  ]
</script>

<div class="page">
  <div class="title">Shortcuts</div>
  <div class="desc-lead">Built-in keybindings. Modifier shown as Ctrl; on macOS that's ⌘.</div>

  <div class="card">
    {#each shortcuts as s, i (s.action)}
      <div class="row">
        <div class="kbd-group" aria-label={s.keys.join(' + ')}>
          {#each s.keys as k (k)}
            <kbd>{k === 'Ctrl' ? '⌃' : k}</kbd>
          {/each}
        </div>
        <div class="action">{s.action}</div>
      </div>
      {#if i < shortcuts.length - 1}<div class="sep"></div>{/if}
    {/each}
  </div>
</div>

<style>
  .title {
    font-family: var(--font-heading);
    font-size: 18px;
    font-weight: 600;
    color: var(--text-primary);
    margin-bottom: 2px;
  }
  .desc-lead {
    color: var(--text-muted);
    font-size: 12.5px;
    margin-bottom: 20px;
  }
  .card {
    background: var(--bg-elevated);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-md);
    overflow: hidden;
    max-width: 560px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 10px 16px;
  }
  .kbd-group {
    display: flex;
    gap: 5px;
    width: 120px;
    flex: none;
  }
  kbd {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-secondary);
    background: var(--bg-base);
    border: 1px solid var(--border-default);
    border-bottom-width: 2px;
    border-radius: 4px;
    padding: 2px 7px;
    min-width: 20px;
    text-align: center;
  }
  .action {
    color: var(--text-primary);
    font-size: 13px;
  }
  .sep {
    height: 1px;
    background: var(--border-subtle);
    margin: 0 16px;
  }
</style>