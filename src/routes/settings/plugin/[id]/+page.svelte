<script lang="ts">
  import { page } from '$app/stores'
  import { onMount } from 'svelte'
  import { plugins, loadPlugins, setEnabled, uninstallPlugin, updatePlugin, checkUpdates } from '../../../../stores/plugins.svelte'
  import { openPluginFolder, getPluginConfig, setPluginConfig } from '../../../../lib/tauri'
  import { pushToast } from '../../../../stores/toast.svelte'
  import SlotStack from '../../../../components/SlotStack.svelte'
  import Toggle from '../../../../components/Toggle.svelte'
  import Button from '../../../../components/Button.svelte'
  import SectionHeader from '../../../../components/SectionHeader.svelte'
  import EmptyState from '../../../../components/EmptyState.svelte'
  import Pill from '../../../../components/Pill.svelte'

  // Design v2 plugin detail (`Pragma UI Kit v2.dc.html` → isSectionPluginDetail):
  // back link, head card with bordered mono initial + status pill + enable
  // toggle, section cards (Skills / plugin settings), and footer actions
  // (Update / Open folder / Uninstall).

  const pluginId = $derived($page.params.id as string)
  const plugin = $derived(plugins.list.find((p) => p.id === pluginId))
  const update = $derived(
    plugins.updates?.find((u) => u.id === pluginId && u.updateAvailable) ?? null
  )
  let busy = $state('') // '' | 'update' | 'uninstall'
  // Plugin config (config.json) — rendered as editable key/value rows below.
  let config = $state<Record<string, unknown>>({})
  let configLoaded = $state(false)
  const saveTimers = new Map<string, ReturnType<typeof setTimeout>>()

  onMount(async () => {
    await loadPlugins()
    if (plugins.checkedAt === null) checkUpdates().catch(() => {})
    config = await getPluginConfig(pluginId)
    configLoaded = true
  })

  function setConfigValue(key: string, value: string) {
    config = { ...config, [key]: value }
    const t = saveTimers.get(key)
    if (t) clearTimeout(t)
    saveTimers.set(
      key,
      setTimeout(() => {
        saveTimers.delete(key)
        setPluginConfig(pluginId, config).catch(() =>
          pushToast('Could not save plugin config', 'error')
        )
      }, 400)
    )
  }

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

  async function doUpdate() {
    if (!plugin) return
    busy = 'update'
    try {
      await updatePlugin(plugin.id)
      pushToast(`Updated ${plugin.name}`, 'success')
      await checkUpdates()
    } catch (err) {
      pushToast(String(err), 'error')
    } finally {
      busy = ''
    }
  }

  async function doUninstall() {
    if (!plugin) return
    busy = 'uninstall'
    try {
      await uninstallPlugin(plugin.id)
      pushToast(`Uninstalled ${plugin.name}`, 'success')
    } catch (err) {
      pushToast(String(err), 'error')
    } finally {
      busy = ''
    }
  }
</script>

<div class="page">
  {#if plugin}
    <Button
      variant="quiet"
      mono
      href="/settings/plugins"
      icon="chevron_left"
      class="crumb"
    >All plugins</Button>

    <!-- head card -->
    <div class="head">
      <span class="initial" aria-hidden="true">{plugin.name.slice(0, 1).toUpperCase()}</span>
      <div class="head-copy">
        <div class="title-row">
          <span class="title">{plugin.name}</span>
          <span class="ver">v{plugin.version}</span>
          <Pill
            tone={!plugin.enabled ? 'muted' : update ? 'warning' : 'success'}
          >
            {!plugin.enabled ? 'disabled' : update ? `update available — v${update.latest}` : 'loaded'}
          </Pill>
        </div>
        <div class="desc-lead">{plugin.description || 'No description'}</div>
      </div>
      <Toggle label="Enable plugin" checked={plugin.enabled} onChange={toggle} />
    </div>

    <!-- details section -->
    <section class="sec">
      <SectionHeader icon="info" title="Details" note="contributed by this plugin" />
      <div class="field"><span class="k">ID</span><span class="v">{plugin.id}</span></div>
      <div class="field"><span class="k">Version</span><span class="v">{plugin.version}</span></div>
      <div class="field"><span class="k">Folder</span><code class="v">{plugin.folder}</code></div>
      <div class="field">
        <span class="k">Contributes</span>
        <span class="v tags">
          {#each plugin.contributes.providers as pr (pr)}<span class="b provider">{pr}</span>{/each}
          {#each plugin.contributes.slots as s (s)}<span class="b slot">{s}</span>{/each}
          {#each plugin.contributes.commands as c (c)}<span class="b command">{c}</span>{/each}
        </span>
      </div>
    </section>

    <!-- skills section: per-skill toggles (skills belong to their plugin) -->
    <section class="sec">
      <SectionHeader icon="auto_awesome" title="Skills" note={plugin.skills.length ? 'enable with the plugin' : 'this plugin contributes none'} />
      {#if plugin.skills.length === 0}
        <div class="no-skills">No skills contributed.</div>
      {:else}
        {#each plugin.skills as sk, i (sk.id)}
          <div class="skill-row" class:last={i === plugin.skills.length - 1} class:dim={!plugin.enabled}>
            <div class="skill-copy">
              <div class="skill-name">{sk.name}</div>
              <div class="skill-desc">{sk.description || '—'}</div>
            </div>
            <Toggle label={`Enable ${sk.name}`} checked={plugin.enabled} onChange={toggle} />
          </div>
        {/each}
      {/if}
    </section>

    <!-- plugin settings: config.json values + the plugin's own screen (slot) -->
    <section class="sec">
      <SectionHeader icon="tune" title="Plugin settings" note="saved to config.json" />
      {#if configLoaded}
        {#if Object.keys(config).length > 0}
          {#each Object.entries(config) as [key, value], i (key)}
            <div class="field cfg" class:last={i === Object.keys(config).length - 1}>
              <span class="k">{key}</span>
              <input
                class="cfg-input"
                value={String(value ?? '')}
                oninput={(e) => setConfigValue(key, (e.target as HTMLInputElement).value)}
                aria-label={key}
              />
            </div>
          {/each}
        {:else}
          <div class="no-skills">No config yet — this plugin's own settings screen (if it provides one) renders below.</div>
        {/if}
      {/if}
      <SlotStack slot="settings-page" context={pluginId} />
    </section>

    <!-- actions -->
    <div class="actions">
      {#if update}
        <Button variant="warning" mono onclick={doUpdate} disabled={busy !== ''}>
          {busy === 'update' ? 'Updating…' : `Update to v${update.latest}`}
        </Button>
      {/if}
      <Button variant="ghost" mono onclick={reveal} disabled={busy !== ''}>Open folder</Button>
      <Button variant="danger" mono onclick={doUninstall} disabled={busy !== ''}>
        {busy === 'uninstall' ? 'Uninstalling…' : 'Uninstall'}
      </Button>
    </div>
  {:else}
    <EmptyState icon="extension">
      <p>Plugin “{pluginId}” not found — is it installed and enabled?</p>
      <a href="/settings/plugins">Back to plugins</a>
    </EmptyState>
  {/if}
</div>

<style>
  /* Back crumb — quiet mono link (class lands on the shared Button). */
  :global(.crumb) {
    padding: 0;
    margin-bottom: 12px;
  }

  /* head card (design pdInitial / pdName / pdStatus / toggle) */
  .head {
    display: flex;
    align-items: center;
    gap: 14px;
    border: 1px solid var(--border-default);
    padding: 14px;
    margin-bottom: 16px;
    background: var(--bg-elevated);
  }
  .initial {
    width: 40px;
    height: 40px;
    flex: none;
    display: flex;
    align-items: center;
    justify-content: center;
    border: 1px solid var(--border-default);
    background: var(--bg-overlay);
    font-family: var(--font-mono);
    font-size: 16px;
    color: var(--accent);
  }
  .head-copy {
    flex: 1;
    min-width: 0;
  }
  .title-row {
    display: flex;
    align-items: center;
    gap: 9px;
    flex-wrap: wrap;
  }
  .title {
    font-family: var(--font-heading);
    font-size: 19px;
    font-weight: 600;
    color: var(--text-primary);
  }
  .ver {
    font-family: var(--font-mono);
    font-size: 10.5px;
    color: var(--text-muted);
  }
  .desc-lead {
    margin-top: 4px;
    color: var(--text-muted);
    font-size: 12.5px;
    text-wrap: pretty;
  }

  /* section cards (design: border card + icon header) */
  .sec {
    border: 1px solid var(--border-default);
    background: var(--bg-elevated);
    margin-bottom: 16px;
    max-width: 680px;
  }
  .field {
    display: flex;
    align-items: baseline;
    gap: 16px;
    padding: 10px 14px;
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
  code.v {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-secondary);
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
  .skill-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    padding: 12px 14px;
    border-bottom: 1px solid var(--border-subtle);
    transition: opacity 0.2s ease;
  }
  .skill-row.last {
    border-bottom: none;
  }
  .skill-row.dim {
    opacity: 0.55;
  }
  .skill-copy {
    flex: 1;
    min-width: 0;
  }
  .skill-name {
    font-family: var(--font-body);
    font-size: 13.5px;
    color: var(--text-primary);
  }
  .skill-desc {
    font-family: var(--font-body);
    font-size: 12px;
    color: var(--text-muted);
    margin-top: 2px;
  }
  .no-skills {
    padding: 12px 14px;
    font-family: var(--font-body);
    font-size: 12px;
    color: var(--text-muted);
  }
  .field.cfg {
    align-items: center;
  }
  .cfg-input {
    flex: 1;
    min-width: 0;
    background: var(--bg-base);
    border: 1px solid var(--border-default);
    padding: 7px 10px;
    font-family: var(--font-mono);
    font-size: 12.5px;
    color: var(--text-primary);
    outline: none;
    transition: border-color 0.18s ease;
  }
  .cfg-input:focus {
    border-color: var(--border-focus);
  }

  .actions {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    max-width: 680px;
  }
</style>
