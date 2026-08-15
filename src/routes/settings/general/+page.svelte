<script lang="ts">
  import Toggle from '../../../components/Toggle.svelte'
  import { getNotificationsEnabled, setNotificationsEnabled } from '../../../lib/tauri'
  import { updater, checkForUpdates, installUpdate } from '../../../stores/updater.svelte'
  import { renderMarkdown } from '../../../lib/markdown'

  let enterSend = $state(true)
  let restoreTabs = $state(true)
  let notifications = $state(true)

  async function checkNow() {
    await checkForUpdates()
  }

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

  <div class="section-title">About &amp; updates</div>

  <div class="card">
    <div class="row">
      <div class="row-text">
        <div class="label">Version</div>
        <div class="desc">Pragma {updater.currentVersion || '—'}</div>
      </div>
      {#if updater.available}
        <span class="status avail" title={`Update available: ${updater.available.version}`}>Update available</span>
      {:else}
        <span class="status">Up to date</span>
      {/if}
    </div>

    {#if updater.available}
      <div class="update-block">
        <div class="update-head">
          <span class="upd-title">Pragma {updater.available.version}</span>
          <span class="upd-actions">
            <button class="ghost" onclick={checkNow} disabled={updater.checking}>
              {updater.checking ? 'Checking…' : 'Check'}
            </button>
            <button class="primary" onclick={installUpdate} disabled={updater.installing || updater.checking}>
              {updater.installing ? 'Downloading…' : 'Install & restart'}
            </button>
          </span>
        </div>
        <div class="changelog">
          {#if updater.available.body}
            {@html renderMarkdown(updater.available.body)}
          {:else}
            <p class="muted">This version’s release notes weren’t published.</p>
          {/if}
        </div>
        {#if updater.failed}<p class="err">Update failed — check your connection and try again.</p>{/if}
      </div>
    {:else}
      <div class="row">
        <div class="row-text">
          <div class="label">Automatic updates</div>
          <div class="desc">Pragma checks for updates silently and notifies you when one is available.</div>
        </div>
        <button class="ghost" onclick={checkNow} disabled={updater.checking}>
          {updater.checking ? 'Checking…' : 'Check for updates'}
        </button>
      </div>
    {/if}
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
  .section-title {
    margin: 22px 0 8px;
    color: var(--text-muted);
    font-size: 12.5px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  .status {
    color: var(--text-muted);
    font-size: 12.5px;
    padding: 3px 11px;
    border: 1px solid var(--border-subtle);
    border-radius: 999px;
    flex: none;
  }
  .status.avail {
    color: var(--accent);
    border-color: var(--accent);
    font-weight: 600;
  }
  .update-block {
    padding: 12px 16px 14px;
    border-top: 1px solid var(--border-subtle);
  }
  .update-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 8px;
  }
  .upd-title { font-size: 13.5px; font-weight: 600; color: var(--text-primary); }
  .upd-actions { display: flex; gap: 8px; flex: none; }
  .changelog {
    max-height: 200px;
    overflow: auto;
    font-size: 12.5px;
    line-height: 1.55;
    color: var(--text-secondary);
    padding: 8px 10px;
    background: var(--bg-base);
    border-radius: var(--radius-md);
  }
  .changelog :global(h1), .changelog :global(h2), .changelog :global(h3) {
    margin: 8px 0 4px; font-size: 13px; font-weight: 600; color: var(--text-primary);
  }
  .changelog :global(ul), .changelog :global(ol) { margin: 4px 0; padding-left: 18px; }
  .changelog :global(li) { margin: 3px 0; }
  .changelog :global(code) { font-family: var(--font-mono); background: oklch(0 0 0/0.25); padding: 0 3px; border-radius: 3px; font-size: 0.92em; }
  .err { color: var(--danger); font-size: 12.5px; margin: 6px 0 0; }
  .ghost, .primary { border: none; border-radius: 6px; padding: 6px 11px; font-size: 12.5px; cursor: pointer; }
  .ghost { background: transparent; color: var(--text-secondary); }
  .ghost:hover { background: var(--bg-overlay); }
  .primary { background: var(--accent); color: var(--accent-on); font-weight: 600; }
  button:disabled { opacity: 0.6; cursor: default; }
  .muted { color: var(--text-muted); }
</style>