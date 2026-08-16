// Central auto-update state for the app.
//
// Two distinct surfaces, per the UX requirements:
//   1. A one-time **changelog dialog** shown at startup when a newer release is
//      out — but only the FIRST time we open the app for that version (skip/
//      dismiss marks the version as seen, persisting in localStorage). The
//      Settings → Update page stays authoritative regardless of skip.
//   2. A **silent 10-minute poller** that never shows a dialog — it just toasts
//      and updates `updater.available` so the Settings → Update path lights up.
import { check, type Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { getVersion } from '@tauri-apps/api/app';
import { pushToast } from './toast.svelte';

export interface AppUpdate {
  version: string;
  body?: string;
}

const LS_KEY = 'pragma:updater';

interface Persisted {
  /** Versions whose startup changelog dialog has been shown/skipped. */
  seenVersions: string[];
}

function loadPersisted(): Persisted {
  const out: Persisted = { seenVersions: [] };
  try {
    const raw = localStorage.getItem(LS_KEY);
    if (raw) {
      const p = JSON.parse(raw) as Partial<Persisted>;
      if (Array.isArray(p.seenVersions)) out.seenVersions = p.seenVersions;
    }
  } catch {
    /* storage unavailable — keep defaults */
  }
  return out;
}
const persisted = loadPersisted();
function persist() {
  try {
    localStorage.setItem(LS_KEY, JSON.stringify(persisted));
  } catch {
    /* storage unavailable */
  }
}

export const updater = $state({
  available: null as AppUpdate | null,
  checking: false,
  installing: false,
  failed: false,
  showDialog: false,
  currentVersion: '',
});

/** Live handle to the plugin's Update object (for downloadAndInstall). */
let live: Update | null = null;

/** Quietly ask the updater plugin whether a newer release exists. */
export async function checkForUpdates(): Promise<AppUpdate | null> {
  if (updater.checking) return updater.available;
  updater.checking = true;
  try {
    const u = await check();
    live = u;
    updater.available = u ? { version: u.version, body: u.body ?? undefined } : null;
    return updater.available;
  } catch {
    return updater.available; // offline / dev build / not configured
  } finally {
    updater.checking = false;
  }
}

export function hasSeen(v: string): boolean {
  return persisted.seenVersions.includes(v);
}

export function markSeen(v: string) {
  if (!persisted.seenVersions.includes(v)) {
    persisted.seenVersions.push(v);
    persist();
  }
}

export function clearSeen(v: string) {
  persisted.seenVersions = persisted.seenVersions.filter((x) => x !== v);
  persist();
}

/** Download + install + relaunch. Sets failed on error so UI can show it. */
export async function installUpdate() {
  if (!live) return;
  updater.installing = true;
  updater.failed = false;
  try {
    await live.downloadAndInstall();
    await relaunch();
  } catch {
    updater.failed = true;
    updater.installing = false;
  }
}

/** Close the startup dialog; treats the current version as seen (skip). */
export function dismissDialog() {
  if (updater.available) markSeen(updater.available.version);
  updater.showDialog = false;
}

async function loadCurrentVersion() {
  try {
    updater.currentVersion = (await getVersion()) || '';
  } catch {
    /* not in a packaged app (dev/browser) */
  }
}

let started = false;
/** Called once from the app shell: load version, run the startup check (which
 *  may show the one-time changelog dialog) and start the silent 10-min poller. */
export function initUpdater() {
  if (started) return;
  started = true;

  loadCurrentVersion();

  checkForUpdates().then((u) => {
    if (u && !hasSeen(u.version)) {
      updater.showDialog = true;
      markSeen(u.version); // one-time — won't nag on subsequent launches
    }
  });

  // 10-minute silent poller: toast + Settings badge only, never a dialog.
  setInterval(
    async () => {
      const before = updater.available?.version;
      const u = await checkForUpdates();
      if (u && u.version !== before) {
        pushToast(`Pragma ${u.version} is available — open Settings → General`, 'info');
      }
    },
    10 * 60 * 1000,
  );
}
