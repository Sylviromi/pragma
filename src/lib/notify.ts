// Native OS notifications for turn completion / errors / approval requests.
//
// Pragma stays quiet when you're already looking at it: notifications are only
// sent while the app window is NOT focused (the in-app tab indicator covers the
// focused case). Pass `force = true` for events that need attention regardless
// of focus, such as a conversation dying on an error.

import { isPermissionGranted, requestPermission, sendNotification } from '@tauri-apps/plugin-notification'
import { getCurrentWindow } from '@tauri-apps/api/window'

let permission: 'granted' | 'denied' | null = null
let checkPromise: Promise<boolean> | null = null

/** Resolve the OS permission once and cache it (some platforms prompt). */
async function hasPermission(): Promise<boolean> {
  if (permission === 'granted') return true
  if (permission === 'denied') return false
  checkPromise ??= (async () => {
    try {
      let granted = await isPermissionGranted()
      if (!granted) granted = (await requestPermission()) === 'granted'
      permission = granted ? 'granted' : 'denied'
      return granted
    } catch {
      permission = 'denied'
      return false
    }
  })()
  return checkPromise
}

/** Send a native notification. Silently no-ops when unsupported or denied. */
export async function notifyOS(title: string, body: string, force = false) {
  try {
    if (!force && (await getCurrentWindow().isFocused())) return
    if (!(await hasPermission())) return
    sendNotification({ title, body })
  } catch {
    // Notifications unavailable on this platform — degrade silently.
  }
}
