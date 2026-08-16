import { THEME_FAMILIES, familyByKey } from '../lib/theme/families';
import { themeVarEntries, type ThemeMode } from '../lib/theme/palette-engine';

const LS_KEY = 'pragma:theme';

export interface ThemePrefs {
  family: string;
  mode: ThemeMode;
  matchSystem: boolean;
  reduceMotion: boolean;
  /** Interface scale in percent — applied as a uniform scale on #app
   *  (90 / 100 / 110 / 125). Default 110: the design's dense mono labels are
   *  too small at 100% on most displays. */
  uiScale: number;
  /** UI font preset — remaps --font-ui/--font-heading/--font-mono together
   *  ('design' = Barlow trio, 'system' = native UI fonts). */
  fontFamily: 'design' | 'system';
}

function loadPrefs(): ThemePrefs {
  const out: ThemePrefs = {
    family: 'steel',
    mode: 'dark',
    matchSystem: false,
    reduceMotion: false,
    uiScale: 110,
    fontFamily: 'design',
  };
  try {
    const raw = localStorage.getItem(LS_KEY);
    if (!raw) return out;
    const parsed: unknown = JSON.parse(raw);
    // Legacy plain-string value ('dark' | 'light' | 'high-contrast') — map it
    // onto the new engine so users keep their old preference.
    if (typeof parsed === 'string') {
      if (parsed === 'light' || parsed === 'mid') out.mode = parsed;
      else if (parsed === 'high-contrast') out.family = 'high-contrast';
      return out;
    }
    const p = parsed as Partial<ThemePrefs>;
    if (typeof p.family === 'string') out.family = p.family;
    if (p.mode === 'light' || p.mode === 'mid' || p.mode === 'dark') out.mode = p.mode;
    if (typeof p.matchSystem === 'boolean') out.matchSystem = p.matchSystem;
    if (typeof p.reduceMotion === 'boolean') out.reduceMotion = p.reduceMotion;
    if (p.uiScale === 90 || p.uiScale === 100 || p.uiScale === 110 || p.uiScale === 125) {
      out.uiScale = p.uiScale;
    }
    if (p.fontFamily === 'design' || p.fontFamily === 'system') out.fontFamily = p.fontFamily;
  } catch {
    /* storage unavailable — keep defaults */
  }
  return out;
}

export const theme = $state<ThemePrefs>(loadPrefs());

function persist() {
  try {
    localStorage.setItem(LS_KEY, JSON.stringify(theme));
  } catch {
    /* storage unavailable */
  }
}

export function setFamily(key: string) {
  theme.family = familyByKey(key).key;
  persist();
}

export function setMode(mode: ThemeMode) {
  theme.mode = mode;
  persist();
}

export function setMatchSystem(v: boolean) {
  theme.matchSystem = v;
  persist();
}

export function setReduceMotion(v: boolean) {
  theme.reduceMotion = v;
  // Components gate animations/transitions on this attribute.
  document.documentElement.setAttribute('data-reduce-motion', v ? 'true' : 'false');
  persist();
}

export function setUiScale(v: number) {
  theme.uiScale = v;
  applyUiScale();
  persist();
}

export function setFontFamily(v: 'design' | 'system') {
  theme.fontFamily = v;
  applyFontFamily();
  persist();
}

/// The one font knob: remaps the trio (ui/heading/mono) so the whole app and
/// plugin surfaces re-skin together. --font-body derives from --font-ui.
const FONT_FAMILIES = {
  design: {
    ui: "'Barlow', system-ui, sans-serif",
    heading: "'Barlow Condensed', system-ui, sans-serif",
    mono: "'JetBrains Mono', ui-monospace, 'SF Mono', Menlo, monospace",
  },
  system: {
    ui: 'system-ui, sans-serif',
    heading: 'system-ui, sans-serif',
    mono: "ui-monospace, 'SF Mono', Menlo, monospace",
  },
} as const;

export function applyFontFamily() {
  const f = FONT_FAMILIES[theme.fontFamily] ?? FONT_FAMILIES.design;
  const el = document.documentElement;
  el.style.setProperty('--font-ui', f.ui);
  el.style.setProperty('--font-heading', f.heading);
  el.style.setProperty('--font-mono', f.mono);
}

/** Reactive scale factor (1 = 100%, 1.25 = 125%) — components that anchor
 *  fixed-position overlays to viewport coordinates divide by this. Call it
 *  inside a reactive context ($derived/$effect) to track scale changes. */
export function uiScaleFactor() {
  return theme.uiScale / 100;
}

/** Apply the interface-scale factor on <html> so every surface (app chrome,
 *  chat, settings, plugin slots) scales uniformly. The actual scaling is a
 *  transform in base.css (#app) — this just publishes the factor as a token. */
export function applyUiScale() {
  document.documentElement.style.setProperty('--ui-scale', String(theme.uiScale / 100));
}

let media: MediaQueryList | null = null;

/// Effective mode after matchSystem resolution, if enabled.
function effectiveMode(): ThemeMode {
  if (!theme.matchSystem) return theme.mode;
  if (!media) media = window.matchMedia('(prefers-color-scheme: light)');
  return media.matches ? 'light' : 'dark';
}

let appliedFamily = '';
let appliedMode: ThemeMode | null = null;

/** Write the active family×mode token set onto <html> so the whole app (and
 *  plugin web components, whose shadow roots inherit custom properties from
 *  their host context) sees the same palette. */
export function applyTheme() {
  const family = familyByKey(theme.family);
  const mode = effectiveMode();
  if (appliedFamily === family.key && appliedMode === mode) return;
  appliedFamily = family.key;
  appliedMode = mode;
  const el = document.documentElement;
  for (const [name, value] of themeVarEntries(family, mode)) {
    el.style.setProperty(`--${name}`, value);
  }
  el.setAttribute('data-theme-family', family.key);
  el.setAttribute('data-theme-mode', mode);
  el.setAttribute('data-reduce-motion', theme.reduceMotion ? 'true' : 'false');
}

export function initTheme() {
  applyTheme();
  applyUiScale();
  applyFontFamily();
  if (!media) media = window.matchMedia('(prefers-color-scheme: light)');
  // Follow OS appearance when matchSystem is on (debounced via requestAnimationFrame).
  const onChange = () => {
    if (theme.matchSystem) {
      requestAnimationFrame(applyTheme);
    }
  };
  media.addEventListener?.('change', onChange);
}

export { THEME_FAMILIES };
