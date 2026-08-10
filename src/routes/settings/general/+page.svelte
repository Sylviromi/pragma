<script lang="ts">
  import Toggle from '../../../components/Toggle.svelte'
  import { getNotificationsEnabled, setNotificationsEnabled } from '../../../lib/tauri'

  let enterSend = $state(true)
  let restoreTabs = $state(true)
  let notifications = $state(true)

  $effect(() => {
    enterSend = localStorage.getItem('pragma:enterSend') !== 'false'
    restoreTabs = localStorage.getItem('pragma:restoreTabs') !== 'false'
    // OS notification preference lives in settings.json (Rust side); sync it in
    // here and fall back to localStorage in the browser mock.
    getNotificationsEnabled().then((v) => {
      notifications = v ?? localStorage.getItem('pragma:notifications') !== 'false'
    })
    if (!localStorage.getItem('pragma:notifications')) {
      localStorage.setItem('pragma:notifications', 'true')
    }
  })

  function toggle(key: string, value: boolean) {
    localStorage.setItem(`pragma:${key}`, String(value))
  }

  function toggleNotifications(value: boolean) {
    toggle('notifications', value)
    setNotificationsEnabled(value)
  }
</script>

<div class="page">
  <div class="title">General</div>
  <div class="desc-lead">Core behavior for the harness.</div>

  <div class="card">
    <div class="row">
      <div class="row-text">
        <div class="label">Send on Enter</div>
        <div class="desc">Shift+Enter always inserts a newline</div>
      </div>
      <Toggle
        label="Send on Enter"
        bind:checked={enterSend}
        onChange={(v) => toggle('enterSend', v)}
      />
    </div>
    <div class="row">
      <div class="row-text">
        <div class="label">Restore tabs on launch</div>
        <div class="desc">Reopen sessions from your last window</div>
      </div>
      <Toggle
        label="Restore tabs on launch"
        bind:checked={restoreTabs}
        onChange={(v) => toggle('restoreTabs', v)}
      />
    </div>
    <div class="row">
      <div class="row-text">
        <div class="label">System notifications</div>
        <div class="desc">OS-level alerts when the agent finishes, asks a question, or errors out — only when the window isn’t focused</div>
      </div>
      <Toggle
        label="System notifications"
        bind:checked={notifications}
        onChange={toggleNotifications}
      />
    </div>
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
    max-width: 640px;
  }
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