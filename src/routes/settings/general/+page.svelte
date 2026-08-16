<script lang="ts">
  import Toggle from '../../../components/Toggle.svelte';
  import SettingsHeader from '../../../components/SettingsHeader.svelte';
  import Card from '../../../components/Card.svelte';
  import SettingRow from '../../../components/SettingRow.svelte';
  import Button from '../../../components/Button.svelte';
  import Changelog from '../../../components/Changelog.svelte';
  import { getNotificationsEnabled, setNotificationsEnabled } from '../../../lib/tauri';
  import { updater, checkForUpdates, installUpdate } from '../../../stores/updater.svelte';

  let enterSend = $state(true);
  let restoreTabs = $state(true);
  let notifications = $state(true);

  async function checkNow() {
    await checkForUpdates();
  }

  $effect(() => {
    enterSend = localStorage.getItem('pragma:enterSend') !== 'false';
    restoreTabs = localStorage.getItem('pragma:restoreTabs') !== 'false';
    // OS notification preference lives in settings.json (Rust side); sync it in
    // here and fall back to localStorage in the browser mock.
    getNotificationsEnabled().then((v) => {
      notifications = v ?? localStorage.getItem('pragma:notifications') !== 'false';
    });
    if (!localStorage.getItem('pragma:notifications')) {
      localStorage.setItem('pragma:notifications', 'true');
    }
  });

  function toggle(key: string, value: boolean) {
    localStorage.setItem(`pragma:${key}`, String(value));
  }

  function toggleNotifications(value: boolean) {
    toggle('notifications', value);
    setNotificationsEnabled(value);
  }
</script>

<div class="page">
  <SettingsHeader
    kicker="Workspace"
    title="General"
    blurb="How sessions start, how agents are allowed to act, and what Pragma sends home."
    meta={`${updater.currentVersion || 'dev'} · ${new Date().getFullYear()}`}
  />

  <Card>
    <SettingRow label="Send on Enter" desc="Shift+Enter always inserts a newline">
      <Toggle
        label="Send on Enter"
        bind:checked={enterSend}
        onChange={(v) => toggle('enterSend', v)}
      />
    </SettingRow>
    <SettingRow label="Restore tabs on launch" desc="Reopen sessions from your last window">
      <Toggle
        label="Restore tabs on launch"
        bind:checked={restoreTabs}
        onChange={(v) => toggle('restoreTabs', v)}
      />
    </SettingRow>
    <SettingRow
      label="System notifications"
      desc="OS-level alerts when the agent finishes, asks a question, or errors out — only when the window isn’t focused"
    >
      <Toggle
        label="System notifications"
        bind:checked={notifications}
        onChange={toggleNotifications}
      />
    </SettingRow>
  </Card>

  <div class="section-title">About &amp; updates</div>

  <Card>
    <SettingRow label="Version" desc={`Pragma ${updater.currentVersion || '—'}`}>
      {#if updater.available}
        <span class="status avail" title={`Update available: ${updater.available.version}`}
          >Update available</span
        >
      {:else}
        <span class="status">Up to date</span>
      {/if}
    </SettingRow>

    {#if updater.available}
      {@const avail = updater.available}
      <div class="update-block">
        <div class="update-head">
          <span class="upd-title">Pragma {avail.version}</span>
          <span class="upd-actions">
            <Button
              variant="ghost"
              flat
              class="gen-btn"
              onclick={checkNow}
              disabled={updater.checking}
            >
              {updater.checking ? 'Checking…' : 'Check'}
            </Button>
            <Button
              variant="primary"
              class="gen-btn"
              onclick={installUpdate}
              disabled={updater.installing || updater.checking}
            >
              {updater.installing ? 'Downloading…' : 'Install & restart'}
            </Button>
          </span>
        </div>
        <div class="changelog-box">
          <Changelog
            body={avail.body ?? ''}
            empty="This version’s release notes weren’t published."
          />
        </div>
        {#if updater.failed}<p class="err">
            Update failed — check your connection and try again.
          </p>{/if}
      </div>
    {:else}
      <SettingRow
        label="Automatic updates"
        desc="Pragma checks for updates silently and notifies you when one is available."
      >
        <Button variant="ghost" flat class="gen-btn" onclick={checkNow} disabled={updater.checking}>
          {updater.checking ? 'Checking…' : 'Check for updates'}
        </Button>
      </SettingRow>
    {/if}
  </Card>
</div>

<style>
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
  .upd-title {
    font-size: 13.5px;
    font-weight: 600;
    color: var(--text-primary);
  }
  .upd-actions {
    display: flex;
    gap: 8px;
    flex: none;
  }
  .changelog-box {
    max-height: 200px;
    overflow: auto;
    font-size: 12.5px;
    line-height: 1.55;
    color: var(--text-secondary);
    padding: 8px 10px;
    background: var(--bg-base);
    border-radius: var(--radius-md);
  }
  /* Restore the page's original changelog heading size. */
  .changelog-box :global(h1),
  .changelog-box :global(h2),
  .changelog-box :global(h3) {
    font-size: 13px;
    color: var(--text-primary);
  }
  .err {
    color: var(--danger);
    font-size: 12.5px;
    margin: 6px 0 0;
  }
  /* Restore the page's original button skin (6px radius, 600 weight). */
  :global(.btn.gen-btn) {
    border-radius: 6px;
    padding: 6px 11px;
    font-weight: 600;
  }
</style>
