import { describe, it, expect } from 'vitest';
import { readFileSync } from 'fs';
import { buildTheme, applyHighContrast, themeVarEntries, BOOTSTRAP_THEME } from '../lib/theme/palette-engine'
import { THEME_FAMILIES, familyByKey } from '../lib/theme/families'

const tokens = readFileSync('src/styles/tokens.css', 'utf-8');

const designTokens = [
  '--bg-base', '--bg-elevated', '--bg-overlay', '--surface-hover',
  '--text-primary', '--text-secondary', '--text-muted',
  '--border-default', '--border-subtle', '--border-focus',
  '--accent', '--accent-soft', '--accent-on', '--accent-hover', '--accent-active',
  '--success', '--warning', '--danger', '--slot',
  '--shadow-sm', '--shadow-md', '--shadow-lg',
  '--radius-sm', '--radius-md', '--radius-lg',
  '--font-heading', '--font-body', '--font-mono'
];

const legacyAliases = [
  '--color-bg-base', '--color-bg-elevated', '--color-bg-overlay',
  '--color-text-primary', '--color-text-secondary', '--color-text-muted',
  '--color-text-accent', '--color-border-default', '--color-border-subtle',
  '--color-border-focus', '--color-accent-brand', '--color-accent-success',
  '--color-accent-warning', '--color-accent-danger', '--font-sans'
];

describe('CSS token system', () => {
  it('tokens.css defines every required design token', () => {
    for (const t of designTokens) expect(tokens).toContain(t);
  });
  it('tokens.css keeps legacy aliases so old components/plugins keep working', () => {
    for (const t of legacyAliases) expect(tokens).toContain(t);
  });
});

describe('theme palette engine', () => {
  it('builds a full token set for every family×mode', () => {
    for (const f of THEME_FAMILIES) {
      for (const mode of ['light', 'mid', 'dark'] as const) {
        const t = f.highContrast
          ? applyHighContrast(buildTheme(f.hue, f.chroma, mode), f.hue, mode)
          : buildTheme(f.hue, f.chroma, mode);
        for (const name of designTokens.filter((k) => !k.startsWith('--radius') && !k.startsWith('--font'))) {
          const key = name.replace(/^--/, '');
          expect(t[key], `${f.key}/${mode} missing ${name}`).toBeTruthy();
        }
      }
    }
  });

  it('themeVarEntries appends static tokens and legacy aliases', () => {
    const entries = themeVarEntries(familyByKey('steel'), 'dark');
    const names = entries.map(([n]) => n);
    expect(names).toContain('bg-base');
    expect(names).toContain('font-body');
    expect(names).toContain('radius-md');
    expect(names).toContain('color-bg-base');
    expect(names).toContain('font-sans');
  });

  it('high-contrast boosts text/surface/border contrast', () => {
    const normal = buildTheme(250, 0.06, 'dark');
    const hc = applyHighContrast(buildTheme(250, 0.06, 'dark'), 250, 'dark');
    expect(hc['bg-base']).not.toBe(normal['bg-base']);
    expect(hc['border-default']).toContain('/ 0.5');
  });

  it('bootstrap theme matches the design reference values', () => {
    // Steel/dark reference (also declared by tokens.css as the first-paint
    // fallback): bg-base = 16% light, 0.033 chroma @ 250 hue; accent = 72%
    // light, 0.055 chroma @ 250 hue.
    expect(oklchParts(BOOTSTRAP_THEME['bg-base'])).toEqual({ l: 16, c: 0.033, h: 250 });
    expect(oklchParts(BOOTSTRAP_THEME.accent)).toEqual({ l: 72, c: 0.055, h: 250 });
  });

  it('tokens.css first-paint values match the engine bootstrap', () => {
    const cssBase = tokens.match(/--bg-base:\s*([^;]+);/)?.[1]?.trim() ?? '';
    const cssAccent = tokens.match(/--accent:\s*([^;]+);/)?.[1]?.trim() ?? '';
    // Same numbers, formatting-agnostic (CSS hand-trims trailing zeros, the
    // engine formats with toFixed(4)) — compare the oklch components.
    expect(oklchParts(cssBase)).toEqual(oklchParts(BOOTSTRAP_THEME['bg-base']));
    expect(oklchParts(cssAccent)).toEqual(oklchParts(BOOTSTRAP_THEME.accent));
  });
});

/// Parse "oklch(L% C H" (alpha ignored) into comparable numbers.
function oklchParts(v: string): { l: number; c: number; h: number } | null {
  const m = v.match(/oklch\(([\d.]+)%\s+([\d.]+)\s+([\d.]+)/);
  return m ? { l: parseFloat(m[1]), c: parseFloat(m[2]), h: parseFloat(m[3]) } : null;
}