import {
  listSessions as fetch,
  createSession as apiCreate,
  archiveSession as apiArchive
} from '../lib/tauri'
import type { Session } from '../lib/types'

export const sessions = $state({
  list: [] as Session[],
  openTabs: [] as string[],
  activeTabId: null as string | null
})

export async function loadSessions() {
  sessions.list = await fetch()
}

export function openTab(id: string) {
  if (!sessions.openTabs.includes(id)) sessions.openTabs = [...sessions.openTabs, id]
  sessions.activeTabId = id
  localStorage.setItem('pragma:lastTabId', id)
}

export function closeTab(id: string) {
  const idx = sessions.openTabs.indexOf(id)
  sessions.openTabs = sessions.openTabs.filter((t) => t !== id)
  if (sessions.activeTabId === id) {
    sessions.activeTabId = sessions.openTabs[Math.max(0, idx - 1)] ?? null
  }
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