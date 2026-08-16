<script lang="ts">
  import { theme, setFamily, setMode, setMatchSystem, setReduceMotion, setUiScale, setFontFamily, THEME_FAMILIES } from '../../../stores/theme.svelte'
  import Toggle from '../../../components/Toggle.svelte'
  import { buildTheme, applyHighContrast } from '../../../lib/theme/palette-engine'
  import type { ThemeMode } from '../../../lib/theme/palette-engine'
  import SettingsHeader from '../../../components/SettingsHeader.svelte'

  // Preview colors for each family card under the current mode — the design
  // shows the actual family swatch (bg base + accent ring), not the active one.
  function previewFor(key: string, mode: ThemeMode): string {
    const f = THEME_FAMILIES.find((x) => x.key === key)!
    const t = f.highContrast
      ? applyHighContrast(buildTheme(f.hue, f.chroma, mode), f.hue, mode)
      : buildTheme(f.hue, f.chroma, mode)
    return `--preview-bg: ${t['bg-base']}; --preview-accent: ${t.accent};`
  }
</script>

<SettingsHeader
    kicker="Interface"
    title="Appearance"
    blurb="Theme, density and the chrome around the conversation."
  />

<div class="label">Theme</div>
<div class="family-row">
  {#each THEME_FAMILIES as f (f.key)}
    <button
      class="family-card"
      class:active={theme.family === f.key}
      onclick={() => setFamily(f.key)}
      aria-pressed={theme.family === f.key}
      style={previewFor(f.key, theme.mode)}
    >
      <span class="swatch"></span>
      {f.name}
    </button>
  {/each}
</div>

<div class="label">Mode</div>
<div class="segmented" role="group" aria-label="Theme mode">
  {#each ['light', 'mid', 'dark'] as m (m)}
    <button
      class="seg"
      class:active={theme.mode === m}
      onclick={() => setMode(m as ThemeMode)}
      aria-pressed={theme.mode === m}
    >{m === 'mid' ? 'Mid' : m[0].toUpperCase() + m.slice(1)}</button>
  {/each}
</div>

<div class="label">Interface scale</div>
<div class="segmented" role="group" aria-label="Interface scale">
  {#each [90, 100, 110, 125] as s (s)}
    <button
      class="seg"
      class:active={theme.uiScale === s}
      onclick={() => setUiScale(s)}
      aria-pressed={theme.uiScale === s}
    >{s}%</button>
  {/each}
</div>

<div class="label">UI font</div>
<div class="segmented" role="group" aria-label="UI font">
  {#each [{ key: 'design', label: 'Design' }, { key: 'system', label: 'System' }] as f (f.key)}
    <button
      class="seg"
      class:active={theme.fontFamily === f.key}
      onclick={() => setFontFamily(f.key as 'design' | 'system')}
      aria-pressed={theme.fontFamily === f.key}
    >{f.label}</button>
  {/each}
</div>
<p class="font-hint">
  Design — Barlow · Barlow Condensed · JetBrains Mono. System — your OS's native UI font.
</p>

<div class="row">
  <div>
    <div class="row-label">Match system appearance</div>
    <div class="row-desc">Follow prefers-color-scheme automatically</div>
  </div>
  <Toggle checked={theme.matchSystem} onChange={setMatchSystem} label="Match system appearance" />
</div>

<div class="row">
  <div>
    <div class="row-label">Reduce motion</div>
    <div class="row-desc">Disable transitions and streaming cursor blink</div>
  </div>
  <Toggle checked={theme.reduceMotion} onChange={setReduceMotion} label="Reduce motion" />
</div>


<style>
  .label {
    font-family: var(--font-body);
    font-size: 12px;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    margin-bottom: 8px;
  }
  .family-row {
    display: flex;
    gap: 8px;
    margin-bottom: 20px;
    flex-wrap: wrap;
  }
  .family-card {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 10px 16px;
    background: var(--bg-elevated);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-md);
    cursor: pointer;
    font-family: var(--font-body);
    font-size: 12px;
    color: var(--text-primary);
  }
  .family-card:hover {
    border-color: var(--text-muted);
  }
  .family-card.active {
    border-color: var(--accent);
  }
  .swatch {
    width: 48px;
    height: 28px;
    border-radius: 2px;
    background: var(--preview-bg);
    border: 1px solid var(--border-default);
    position: relative;
    box-shadow: inset 0 0 0 3px var(--preview-accent);
  }
  .segmented {
    display: flex;
    gap: 2px;
    border: 1px solid var(--border-default);
    border-radius: 4px;
    overflow: hidden;
    width: fit-content;
    margin-bottom: 20px;
  }
  .seg {
    padding: 6px 12px;
    font-size: 11.5px;
    font-family: var(--font-body);
    border: none;
    cursor: pointer;
    background: transparent;
    color: var(--text-secondary);
  }
  .seg.active {
    background: var(--accent);
    color: var(--accent-on);
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 20px;
    padding: 14px 0;
    border-top: 1px solid var(--border-subtle);
  }
  .row-label {
    font-family: var(--font-body);
    font-size: 14px;
    color: var(--text-primary);
  }
  .row-desc {
    font-family: var(--font-body);
    font-size: 12px;
    color: var(--text-muted);
    margin-top: 2px;
  }
  .font-hint {
    margin-top: 6px;
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-muted);
  }
</style>