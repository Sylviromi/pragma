import { THEME_FAMILIES, familyByKey } from '../lib/theme/families'
import { themeVarEntries, type ThemeMode } from '../lib/theme/palette-engine'

const LS_KEY = 'pragma:theme'

export interface ThemePrefs {
  family: string
  mode: ThemeMode
  matchSystem: boolean
  reduceMotion: boolean
}

function loadPrefs(): ThemePrefs {
  const out: ThemePrefs = { family: 'steel', mode: 'dark', matchSystem: false, reduceMotion: false }
  try {
    const raw = localStorage.getItem(LS_KEY)
    if (!raw) return out
    const parsed: unknown = JSON.parse(raw)
    // Legacy plain-string value ('dark' | 'light' | 'high-contrast') — map it
    // onto the new engine so users keep their old preference.
    if (typeof parsed === 'string') {
      if (parsed === 'light' || parsed === 'mid') out.mode = parsed
      else if (parsed === 'high-contrast') out.family = 'high-contrast'
      return out
    }
    const p = parsed as Partial<ThemePrefs>
    if (typeof p.family === 'string') out.family = p.family
    if (p.mode === 'light' || p.mode === 'mid' || p.mode === 'dark') out.mode = p.mode
    if (typeof p.matchSystem === 'boolean') out.matchSystem = p.matchSystem
    if (typeof p.reduceMotion === 'boolean') out.reduceMotion = p.reduceMotion
  } catch {
    /* storage unavailable — keep defaults */
  }
  return out
}

export const theme = $state<ThemePrefs>(loadPrefs())

function persist() {
  try {
    localStorage.setItem(LS_KEY, JSON.stringify(theme))
  } catch {
    /* storage unavailable */
  }
}

export function setFamily(key: string) {
  theme.family = familyByKey(key).key
  persist()
}

export function setMode(mode: ThemeMode) {
  theme.mode = mode
  persist()
}

export function setMatchSystem(v: boolean) {
  theme.matchSystem = v
  persist()
}

export function setReduceMotion(v: boolean) {
  theme.reduceMotion = v
  // Components gate animations/transitions on this attribute.
  document.documentElement.setAttribute('data-reduce-motion', v ? 'true' : 'false')
  persist()
}

let media: MediaQueryList | null = null

/// Effective mode after matchSystem resolution, if enabled.
function effectiveMode(): ThemeMode {
  if (!theme.matchSystem) return theme.mode
  if (!media) media = window.matchMedia('(prefers-color-scheme: light)')
  return media.matches ? 'light' : 'dark'
}

let appliedFamily = ''
let appliedMode: ThemeMode | null = null

/** Write the active family×mode token set onto <html> so the whole app (and
 *  plugin web components, whose shadow roots inherit custom properties from
 *  their host context) sees the same palette. */
export function applyTheme() {
  const family = familyByKey(theme.family)
  const mode = effectiveMode()
  if (appliedFamily === family.key && appliedMode === mode) return
  appliedFamily = family.key
  appliedMode = mode
  const el = document.documentElement
  for (const [name, value] of themeVarEntries(family, mode)) {
    el.style.setProperty(`--${name}`, value)
  }
  el.setAttribute('data-theme-family', family.key)
  el.setAttribute('data-theme-mode', mode)
  el.setAttribute('data-reduce-motion', theme.reduceMotion ? 'true' : 'false')
}

export function initTheme() {
  applyTheme()
  if (!media) media = window.matchMedia('(prefers-color-scheme: light)')
  // Follow OS appearance when matchSystem is on (debounced via requestAnimationFrame).
  const onChange = () => {
    if (theme.matchSystem) {
      requestAnimationFrame(applyTheme)
    }
  }
  media.addEventListener?.('change', onChange)
}

export { THEME_FAMILIES }