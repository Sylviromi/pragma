import { listen } from '@tauri-apps/api/event'
import {
  listPlugins as apiListPlugins,
  setPluginEnabled as apiSetEnabled,
  readPluginSkill as apiReadSkill
} from '../lib/tauri'
import type { PluginInfo } from '../lib/tauri'
import { loadProviders, initProviderEvents } from './providers.svelte'

export const plugins = $state({
  list: [] as PluginInfo[],
  /** Bump per plugin id (or '*' for all) to force re-import of component modules. */
  cacheKey: {} as Record<string, number>,
  loading: false
})

export async function loadPlugins() {
  plugins.loading = true
  try {
    plugins.list = await apiListPlugins()
  } finally {
    plugins.loading = false
  }
}

export function bumpPlugin(id: string) {
  plugins.cacheKey[id] = Date.now()
}

export function bumpAll() {
  plugins.cacheKey['*'] = Date.now()
}

export function cacheKeyFor(id: string): number {
  return Math.max(plugins.cacheKey[id] ?? 0, plugins.cacheKey['*'] ?? 0)
}

export async function setEnabled(id: string, enabled: boolean) {
  await apiSetEnabled(id, enabled)
  await loadPlugins()
  bumpPlugin(id)
}

export async function skillFor(id: string): Promise<string> {
  try {
    return await apiReadSkill(id)
  } catch {
    return ''
  }
}

/** Plugins that claim a given slot and can actually mount a component. */
export function pluginsForSlot(slot: string): PluginInfo[] {
  return plugins.list.filter(
    (p) => p.enabled && p.hasComponent && p.contributes.slots.includes(slot)
  )
}

let initialized = false
export function initPluginEvents() {
  if (initialized) return
  initialized = true
  // Model discovery (Anthropic /v1/models, /v1/models of OpenAI-compat
  // providers) resolves asynchronously — refresh the picker when it lands.
  initProviderEvents()
  // Backend reload (file watcher) → re-import all plugin components AND refresh
  // the provider/model list. Without the provider refresh the picker keeps
  // showing a stale list after a plugin re-registers (which is what made it
  // appear to "switch" the active model on hot reload).
  listen('plugin:reload', async () => {
    await loadPlugins()
    bumpAll()
    await loadProviders()
  })
}