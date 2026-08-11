<script lang="ts">
  import { onMount } from 'svelte'
  import { plugins, loadPlugins, setEnabled } from '../../../stores/plugins.svelte'
  import { listSkills } from '../../../lib/tauri'
  import type { SkillInfo } from '../../../lib/types'
  import Toggle from '../../../components/Toggle.svelte'
  import Icon from '../../../components/Icon.svelte'
  import { pushToast } from '../../../stores/toast.svelte'

  let skillRows = $state<SkillInfo[]>([])

  onMount(async () => {
    await loadPlugins()
    skillRows = await listSkills()
  })

  function skillEnabled(skill: SkillInfo): boolean {
    if (!skill.pluginId) return true // native skills are always on
    return plugins.list.find((p) => p.id === skill.pluginId)?.enabled ?? false
  }

  async function toggleRow(skill: SkillInfo) {
    if (!skill.pluginId) return
    const next = !skillEnabled(skill)
    try {
      await setEnabled(skill.pluginId, next)
      pushToast(`${next ? 'Enabled' : 'Disabled'} ${skill.id}`, 'success')
      skillRows = await listSkills()
    } catch (err) {
      pushToast(String(err), 'error')
    }
  }

  function sourceLabel(source: string) {
    return source === 'native' ? 'built-in' : source === 'plugin-skill' ? 'plugin skill' : 'plugin'
  }
</script>

<div class="page">
  <div class="title">Skills</div>
  <div class="desc-lead">
    The model loads skills on demand with <code>load_skill</code> — loading a skill injects its
    guidance and adds its tools to the session. Plugins ship skills as
    <code>skills/&lt;id&gt;/skill.md</code> (+ optional <code>tools.json</code> / <code>handler.js</code>).
  </div>

  {#if skillRows.length === 0}
    <div class="empty">
      <Icon name="neurology" size={30} />
      <p>No skills installed yet.</p>
    </div>
  {:else}
    <div class="card">
      {#each skillRows as skill (skill.id)}
        <div class="row">
          <div class="row-text">
            <div class="label">{skill.name}<span class="id"> · {skill.id}</span></div>
            <div class="desc">{skill.description}</div>
            <div class="badges">
              <span class="b source">{sourceLabel(skill.source)}</span>
              {#if skill.hasTools}<span class="b tools">tools</span>{/if}
              {#if skill.hasHandler}<span class="b handler">sidecar</span>{/if}
            </div>
          </div>
          {#if skill.pluginId}
            <Toggle
              label={`Enable ${skill.id}`}
              checked={skillEnabled(skill)}
              onChange={() => toggleRow(skill)}
            />
          {:else}
            <span class="always">always available</span>
          {/if}
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
  .label .id {
    color: var(--text-muted);
    font-size: 11.5px;
  }
  .desc {
    color: var(--text-muted);
    font-size: 12px;
    margin-top: 1px;
  }
  .badges {
    display: flex;
    gap: 6px;
    margin-top: 6px;
  }
  .b {
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-muted);
    background: var(--bg-overlay);
    border: 1px solid var(--border-subtle);
    border-radius: 3px;
    padding: 1px 6px;
  }
  .b.tools     { color: var(--accent); }
  .b.handler   { color: var(--warning); }
  .always {
    color: var(--text-muted);
    font-size: 11.5px;
  }
</style>