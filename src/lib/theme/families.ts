import type { ThemeFamily } from './palette-engine'

/// Theme families available in the picker (Settings → Appearance).
/// The first is the default. `highContrast` is authored on top of the family
/// hues below (Steel) so it keeps a coherent identity while maximizing contrast.
export const THEME_FAMILIES: ThemeFamily[] = [
  // Default theme — honors the design's Steel family (hue 250, low chroma).
  { key: 'steel', name: 'Steel', hue: 250, chroma: 0.055 },
  { key: 'forest', name: 'Forest', hue: 150, chroma: 0.09 },
  { key: 'amber', name: 'Amber', hue: 70, chroma: 0.13 },
  { key: 'violet', name: 'Violet', hue: 300, chroma: 0.1 },
  // Authored for accessibility: max text/surface contrast, full borders.
  { key: 'high-contrast', name: 'High Contrast', hue: 250, chroma: 0.06, highContrast: true },
]

export function familyByKey(key: string | null | undefined): ThemeFamily {
  return THEME_FAMILIES.find((f) => f.key === key) ?? THEME_FAMILIES[0]
}