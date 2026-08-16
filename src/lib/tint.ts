import { theme } from '../stores/theme.svelte'

/// Deterministic per-session accent tint — design `tintFor(id)` (Pragma UI Kit
/// v2): FNV-ish hash of the id → golden-angle hue on a fixed lightness, so each
/// session gets a stable, distinguishable color that still reads as part of the
/// theme. Used for rail dots/borders, the tab-bar status dot and the agent
/// mark avatar.
function hueFor(seed: string): number {
  let h = 0
  for (const c of String(seed ?? '')) h = (h * 131 + c.charCodeAt(0)) | 0
  return (Math.abs(h) * 137.508) % 360
}

/** Per-session tint (oklch string). Dark/mid modes use the lighter value. */
export function sessionTint(id: string | null | undefined): string {
  const hue = hueFor(id ?? '')
  const dark = theme.mode !== 'light'
  return `oklch(${dark ? 0.74 : 0.58} 0.11 ${hue.toFixed(1)})`
}

/** Tint for sub-agents / nodes keyed by name (design `hueTint`). */
export function hueTint(name: string | null | undefined): string {
  const hue = hueFor(name ?? '')
  const dark = theme.mode !== 'light'
  return `oklch(${dark ? 0.76 : 0.55} 0.12 ${hue.toFixed(1)})`
}
