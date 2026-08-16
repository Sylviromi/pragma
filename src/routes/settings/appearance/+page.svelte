<script lang="ts">
  import { theme, setFamily, setMode, setMatchSystem, setReduceMotion, setUiScale, setFontFamily, THEME_FAMILIES } from '../../../stores/theme.svelte'
  import Toggle from '../../../components/Toggle.svelte'
  import SettingRow from '../../../components/SettingRow.svelte'
  import Segmented from '../../../components/Segmented.svelte'
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
<div class="seg-block">
  <Segmented
    options={[
      { value: 'light', label: 'Light' },
      { value: 'mid', label: 'Mid' },
      { value: 'dark', label: 'Dark' },
    ]}
    value={theme.mode}
    onchange={(m) => setMode(m as ThemeMode)}
    ariaLabel="Theme mode"
  />
</div>

<div class="label">Interface scale</div>
<div class="seg-block">
  <Segmented
    options={[90, 100, 110, 125].map((s) => ({ value: String(s), label: `${s}%` }))}
    value={String(theme.uiScale)}
    onchange={(v) => setUiScale(Number(v))}
    ariaLabel="Interface scale"
  />
</div>

<div class="label">UI font</div>
<div class="seg-block">
  <Segmented
    options={[
      { value: 'design', label: 'Design' },
      { value: 'system', label: 'System' },
    ]}
    value={theme.fontFamily}
    onchange={(f) => setFontFamily(f as 'design' | 'system')}
    ariaLabel="UI font"
  />
</div>
<p class="font-hint">
  Design — Barlow · Barlow Condensed · JetBrains Mono. System — your OS's native UI font.
</p>

<SettingRow bare label="Match system appearance" desc="Follow prefers-color-scheme automatically">
  <Toggle checked={theme.matchSystem} onChange={setMatchSystem} label="Match system appearance" />
</SettingRow>

<SettingRow bare label="Reduce motion" desc="Disable transitions and streaming cursor blink">
  <Toggle checked={theme.reduceMotion} onChange={setReduceMotion} label="Reduce motion" />
</SettingRow>


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
  .seg-block {
    margin-bottom: 20px;
  }
  .font-hint {
    margin-top: 6px;
    margin-bottom: 20px;
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-muted);
  }
</style>
