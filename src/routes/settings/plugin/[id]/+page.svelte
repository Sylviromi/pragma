<script lang="ts">
  import { page } from '$app/stores'
  import { onMount } from 'svelte'
  import { plugins, loadPlugins, setEnabled } from '../../../../stores/plugins.svelte'
  import { openPluginFolder } from '../../../../lib/tauri'
  import { pushToast } from '../../../../stores/toast.svelte'
  import SlotStack from '../../../../components/SlotStack.svelte'
  import Toggle from '../../../../components/Toggle.svelte'
  import Icon from '../../../../components/Icon.svelte'

  const pluginId = $derived($page.params.id as string)
  const plugin = $derived(plugins.list.find((p) => p.id === pluginId))

  onMount(async () => {
    await loadPlugins()
  })

  async function toggle() {
    if (!plugin) return
    const next = !plugin.enabled
    try {
      await setEnabled(plugin.id, next)
      pushToast(`${next ? 'Enabled' : 'Disabled'} ${plugin.name}`, 'success')
    } catch (err) {
      pushToast(String(err), 'error')
    }
  }

  function reveal() {
    openPluginFolder(pluginId).catch((e) => pushToast(String(e), 'error'))
  }
</script>

<div class="page">
  {#if plugin}
    <a class="crumb" href="/settings/plugins">
      <Icon name="chevron_left" size={14} /> Plugins
    </a>

    <div class="head">
      <span class="initial" aria-hidden="true">{plugin.name.slice(0, 1).toUpperCase()}</span>
      <div>
        <div class="title">
          {plugin.name}
          <span class="ver">v{plugin.version}</span>
        </div>
        <div class="desc-lead">{plugin.description || 'No description'}</div>
      </div>
      <Toggle label="Enable plugin" checked={plugin.enabled} onChange={toggle} />
    </div>

    <div class="card fields">
      <div class="field"><span class="k">ID</span><span class="v">{plugin.id}</span></div>
      <div class="field"><span class="k">Version</span><span class="v">{plugin.version}</span></div>
      <div class="field"><span class="k">Status</span><span class="v"><span class="dot" class:off={!plugin.enabled}></span> {plugin.enabled ? 'Active' : 'Disabled'}</span></div>
      <div class="field"><span class="k">Folder</span><code class="v">{plugin.folder}</code></div>
      <div class="field">
        <span class="k">Contributes</span>
        <span class="v tags">
          {#each plugin.contributes.providers as pr (pr)}<span class="b provider">{pr}</span>{/each}
          {#each plugin.contributes.slots as s (s)}<span class="b slot">{s}</span>{/each}
          {#each plugin.contributes.commands as c (c)}<span class="b command">{c}</span>{/each}
          {#if plugin.hasSkill}<span class="b skill">skill</span>{/if}
        </span>
      </div>
    </div>

    <div class="actions">
      <button class="ghost" onclick={reveal}>Open folder</button>
    </div>

    <!-- Plugin-provided settings screen (slot contract: settings-page). -->
    <SlotStack slot="settings-page" context={pluginId} />
  {:else}
    <div class="missing">
      <Icon name="extension" size={28} />
      <p>Plugin “{pluginId}” not found — is it installed and enabled?</p>
      <a href="/settings/plugins">Back to plugins</a>
    </div>
  {/if}
</div>

<style>
  .crumb {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    color: var(--text-muted);
    font-size: 12px;
    text-decoration: none;
    margin-bottom: 14px;
  }
  .crumb:hover {
    color: var(--text-primary);
  }
  .head {
    display: flex;
    align-items: center;
    gap: 14px;
    margin-bottom: 18px;
  }
  .head > div:nth-child(2) {
    flex: 1;
    min-width: 0;
  }
  .initial {
    width: 40px;
    height: 40px;
    border-radius: 5px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-family: var(--font-heading);
    font-weight: 600;
    font-size: 18px;
    color: var(--accent-on);
    background: var(--accent);
    flex: none;
  }
  .title {
    font-family: var(--font-heading);
    font-size: 18px;
    font-weight: 600;
    color: var(--text-primary);
    display: flex;
    align-items: baseline;
    gap: 8px;
  }
  .ver {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-muted);
  }
  .desc-lead {
    color: var(--text-muted);
    font-size: 12.5px;
  }
  .card {
    background: var(--bg-elevated);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-md);
    overflow: hidden;
    max-width: 640px;
  }
  .fields {
    margin-bottom: 14px;
  }
  .field {
    display: flex;
    align-items: baseline;
    gap: 16px;
    padding: 10px 16px;
    border-bottom: 1px solid var(--border-subtle);
    font-size: 12.5px;
  }
  .field:last-child {
    border-bottom: none;
  }
  .k {
    width: 110px;
    flex: none;
    font-family: var(--font-mono);
    font-size: 10.5px;
    letter-spacing: 0.06em;
    color: var(--text-muted);
  }
  .v {
    color: var(--text-primary);
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .field code.v {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-secondary);
  }
  .dot {
    display: inline-block;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--success);
    margin-right: 4px;
  }
  .dot.off {
    background: var(--text-muted);
  }
  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .b {
    font-family: var(--font-mono);
    font-size: 10px;
    border: 1px solid var(--border-default);
    border-radius: 2px;
    padding: 1px 7px;
  }
  .b.provider {
    color: var(--success);
    border-color: color-mix(in srgb, var(--success) 40%, transparent);
    background: color-mix(in srgb, var(--success) 12%, transparent);
  }
  .b.slot {
    color: var(--accent);
  }
  .b.command {
    color: var(--slot);
  }
  .b.skill {
    color: var(--warning);
  }
  .actions {
    display: flex;
    gap: 10px;
    margin-bottom: 18px;
  }
  .ghost {
    background: transparent;
    color: var(--text-primary);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-sm);
    padding: 6px 14px;
    font-size: 12px;
    cursor: pointer;
  }
  .ghost:hover {
    border-color: var(--text-muted);
  }
  .missing {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    padding: 48px 0;
    color: var(--text-muted);
    font-size: 12.5px;
  }
  .missing p {
    margin: 0;
  }
  .missing a {
    color: var(--accent);
    font-size: 12px;
  }
</style>