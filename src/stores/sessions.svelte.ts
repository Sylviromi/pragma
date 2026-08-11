import {
  listSessions as fetch,
  createSession as apiCreate,
  listenSessionRenamed
} from '../lib/tauri'
import type { Session } from '../lib/types'
import { isBrowserMock } from '../lib/mock'

export const sessions = $state({
  list: [] as Session[],
  openTabs: [] as string[],
  activeTabId: null as string | null
})

const LS_TABS = 'pragma:openTabs'
const LS_ACTIVE = 'pragma:activeTabId'

/** Persist the full open-tab set so restarts restore every session tab. */
function persistTabs() {
  try {
    localStorage.setItem(LS_TABS, JSON.stringify(sessions.openTabs))
    if (sessions.activeTabId) {
      localStorage.setItem(LS_ACTIVE, sessions.activeTabId)
    } else {
      localStorage.removeItem(LS_ACTIVE)
    }
  } catch { /* storage unavailable — ignore */ }
}

let _renamedUnlisten: (() => void) | null = null

/** Keep in-app session names in sync when the backend auto-names a fresh
 *  session from its first prompt (tabs + sidebar derive from sessions.list).
 *  Idempotent; the listener is torn down on HMR like chat.svelte.ts does. */
export async function initSessionEvents() {
  if (_renamedUnlisten) return
  _renamedUnlisten = await listenSessionRenamed(({ id, name }) => {
    const s = sessions.list.find((x) => x.id === id)
    if (s && name && s.name !== name) s.name = name
  })
}
if (import.meta.hot) {
  import.meta.hot.dispose(() => {
    _renamedUnlisten?.()
    _renamedUnlisten = null
  })
}

export async function loadSessions() {
  sessions.list = await fetch()
  await initSessionEvents()
}

/** Restore the previously-open tabs (and active tab) after a restart.
 *  Must run after `loadSessions()` so ids can be validated against the DB. */
export function restoreTabs() {
  // User toggle "Restore tabs on launch" (settings/general) — default on.
  const restore = localStorage.getItem('pragma:restoreTabs') !== 'false'
  try {
    const raw = restore ? localStorage.getItem(LS_TABS) : null
    const saved: unknown = raw ? JSON.parse(raw) : []
    const ids = Array.isArray(saved) ? saved.filter((x): x is string => typeof x === 'string') : []
    // Only reopen sessions that still exist and aren't archived.
    sessions.openTabs = ids.filter((id) => sessions.list.some((s) => s.id === id))
    const active = localStorage.getItem(LS_ACTIVE)
    sessions.activeTabId =
      (active && sessions.openTabs.includes(active)) ? active : (sessions.openTabs[0] ?? null)
  } catch {
    sessions.openTabs = []
    sessions.activeTabId = null
  }
  // Fallback: nothing was restored (fresh launch, cleared storage, or the
  // saved tabs no longer exist) — auto-open the most recent session so the
  // full history is visible immediately instead of requiring a first message.
  if (restore && sessions.openTabs.length === 0 && sessions.list.length > 0) {
    openTab(sessions.list[0].id)
  }
  // Browser mock: no real tabs exist — auto-open the first mock session so
  // the full UI (chat, composer, sidebar) is exerciseable in a plain browser.
  if (isBrowserMock && sessions.openTabs.length === 0 && sessions.list.length > 0) {
    openTab(sessions.list[0].id)
  }
}

export function openTab(id: string) {
  if (!sessions.openTabs.includes(id)) sessions.openTabs = [...sessions.openTabs, id]
  sessions.activeTabId = id
  persistTabs()
}

export function closeTab(id: string) {
  const idx = sessions.openTabs.indexOf(id)
  sessions.openTabs = sessions.openTabs.filter((t) => t !== id)
  if (sessions.activeTabId === id) {
    sessions.activeTabId = sessions.openTabs[Math.max(0, idx - 1)] ?? null
  }
  persistTabs()
}

export async function newSession(name?: string) {
  const s = await apiCreate(name ?? `Session ${new Date().toLocaleTimeString()}`)
  sessions.list = [s, ...sessions.list]
  openTab(s.id)
  return s
}

export function getSession(id: string) {
  return sessions.list.find((s) => s.id === id)
}