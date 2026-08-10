<script lang="ts">
  import { onMount } from 'svelte'
  import { plugins, loadPlugins, setEnabled } from '../../../stores/plugins.svelte'
  import Toggle from '../../../components/Toggle.svelte'
  import Icon from '../../../components/Icon.svelte'
  import { pushToast } from '../../../stores/toast.svelte'

  let skillRows = $state<
    { id: string; name: string; enabled: boolean; source: string }[]
  >([])

  onMount(async () => {
    await loadPlugins()
    skillRows = plugins.list
      .filter((p) => p.hasSkill)
      .map((p) => ({
        id: p.id,
        name: p.name,
        enabled: p.enabled,
        source: 'plugin'
      }))
  })

  async function toggleRow(row: { id: string; enabled: boolean }) {
    const next = !row.enabled
    try {
      await setEnabled(row.id, next)
      const cur = skillRows.find((r) => r.id === row.id)
      if (cur) cur.enabled = next
      pushToast(`${next ? 'Enabled' : 'Disabled'} ${row.id} skill`, 'success')
    } catch (err) {
      pushToast(String(err), 'error')
    }
  }
</script>

<div class="page">
  <div class="title">Skills</div>
  <div class="desc-lead">
    Skills are injected into every conversation as system context. Include one via
    <code>manifest.json → contributes.skills</code>.
  </div>

  {#if skillRows.length === 0}
    <div class="empty">
      <Icon name="neurology" size={30} />
      <p>No skills installed yet.</p>
    </div>
  {:else}
    <div class="card">
      {#each skillRows as row (row.id)}
        <div class="row">
          <div class="row-text">
            <div class="label">{row.name}</div>
            <div class="desc">injected · {row.source} skill</div>
          </div>
          <Toggle label={`Enable ${row.name}`} checked={row.enabled} onChange={(v) => toggleRow(row)} />
        </div>
      {/each}
    </div>
  {/if}
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
  .desc-lead code {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-secondary);
    background: var(--bg-overlay);
    padding: 1px 5px;
    border-radius: 2px;
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 48px 0;
    color: var(--text-muted);
    font-size: 12.5px;
  }
  .empty p {
    margin: 0;
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