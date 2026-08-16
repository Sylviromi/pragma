import { listen } from '@tauri-apps/api/event';
import {
  listPlugins as apiListPlugins,
  setPluginEnabled as apiSetEnabled,
  installPlugin as apiInstallPlugin,
  uninstallPlugin as apiUninstallPlugin,
  checkPluginUpdates as apiCheckUpdates,
  updatePlugin as apiUpdatePlugin,
} from '../lib/tauri';
import type { PluginInfo, PluginUpdateInfo } from '../lib/tauri';
import { isBrowserMock } from '../lib/mock';
import { loadProviders, initProviderEvents } from './providers.svelte';

export const plugins = $state({
  list: [] as PluginInfo[],
  /** Bump per plugin id (or '*' for all) to force re-import of component modules. */
  cacheKey: {} as Record<string, number>,
  loading: false,
  /** Last update-check result (per plugin), null until the first check. */
  updates: null as PluginUpdateInfo[] | null,
  /** When the last check ran — gates the startup auto-check freshness. */
  checkedAt: null as number | null,
});

export async function loadPlugins() {
  plugins.loading = true;
  try {
    plugins.list = await apiListPlugins();
  } finally {
    plugins.loading = false;
  }
}

function bumpPlugin(id: string) {
  plugins.cacheKey[id] = Date.now();
}

function bumpAll() {
  plugins.cacheKey['*'] = Date.now();
}

export function cacheKeyFor(id: string): number {
  return Math.max(plugins.cacheKey[id] ?? 0, plugins.cacheKey['*'] ?? 0);
}

export async function setEnabled(id: string, enabled: boolean) {
  await apiSetEnabled(id, enabled);
  await loadPlugins();
  bumpPlugin(id);
}

/** Install from a source spec (npm:… or a local folder path) — backend fetches,
 *  validates and enables. */
export async function installFromSource(spec: string) {
  await apiInstallPlugin(spec);
  await loadPlugins();
  bumpAll();
  await loadProviders();
}

/** Remove an installed plugin. */
export async function uninstallPlugin(id: string) {
  await apiUninstallPlugin(id);
  await loadPlugins();
  bumpAll();
  await loadProviders();
}

/** Poll recorded sources for newer versions (npm latest). Stores
 *  the result on the store so badges render reactively; dedupes concurrent
 *  callers (startup auto-check + manual button + page mount). */
let checkingUpdates = false;
export async function checkUpdates(): Promise<PluginUpdateInfo[]> {
  if (checkingUpdates) return plugins.updates ?? [];
  checkingUpdates = true;
  try {
    const updates = await apiCheckUpdates();
    plugins.updates = updates;
    plugins.checkedAt = Date.now();
    await loadPlugins();
    return updates;
  } finally {
    checkingUpdates = false;
  }
}

/** Re-check at most every 10 minutes; silent (no toast) — startup checks must
 *  never interrupt, the manual button still reports errors. */
const AUTO_CHECK_MS = 10 * 60 * 1000;

export async function autoCheckUpdates() {
  if (isBrowserMock) return;
  if (plugins.checkedAt && Date.now() - plugins.checkedAt < AUTO_CHECK_MS) return;
  try {
    await checkUpdates();
  } catch {
    /* silent — checkedAt stays null so the next startup retries */
  }
}

/** Update one plugin to the latest version from its recorded source. */
export async function updatePlugin(id: string) {
  await apiUpdatePlugin(id);
  await loadPlugins();
  bumpAll();
  await loadProviders();
}

/** Plugins that claim a given slot and can actually mount a component. */
export function pluginsForSlot(slot: string): PluginInfo[] {
  return plugins.list.filter(
    (p) => p.enabled && p.hasComponent && p.contributes.slots.includes(slot),
  );
}

let initialized = false;
export function initPluginEvents() {
  if (initialized) return;
  initialized = true;
  // Model discovery (Anthropic /v1/models, /v1/models of OpenAI-compat
  // providers) resolves asynchronously — refresh the picker when it lands.
  initProviderEvents();
  // Backend reload (file watcher) → re-import all plugin components AND refresh
  // the provider/model list. Without the provider refresh the picker keeps
  // showing a stale list after a plugin re-registers (which is what made it
  // appear to "switch" the active model on hot reload).
  listen('plugin:reload', async () => {
    await loadPlugins();
    bumpAll();
    await loadProviders();
  });
}
