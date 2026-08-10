import {
  listSessions as fetch,
  createSession as apiCreate,
  archiveSession as apiArchive
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

export async function loadSessions() {
  sessions.list = await fetch()
}

/** Restore the previously-open tabs (and active tab) after a restart.
 *  Must run after `loadSessions()` so ids can be validated against the DB. */
export function restoreTabs() {
  try {
    const raw = localStorage.getItem(LS_TABS)
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

export async function archiveTab(id: string) {
  await apiArchive(id)
  closeTab(id)
  sessions.list = sessions.list.filter((s) => s.id !== id)
}

export function getSession(id: string) {
  return sessions.list.find((s) => s.id === id)
}