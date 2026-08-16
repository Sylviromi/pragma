export type ToastKind = 'info' | 'success' | 'error' | 'warning' | 'progress'

export interface ToastAction {
  label: string
  onClick?: () => void
}

export interface ToastItem {
  id: number
  /** Primary line (kept as `message` for backward compat with pushToast). */
  message: string
  kind: ToastKind
  /** Optional secondary line. */
  desc?: string
  /** Mono source tag (e.g. "plugins · reload"). */
  source?: string
  actions?: ToastAction[]
  /** Lifetime ms before auto-dismiss; 0 = sticky. */
  life?: number
}

export const toasts = $state<ToastItem[]>([])

let nextId = 1

/** Push a toast. `pushToast(message, kind)` keeps working; richer payloads
 *  pass a ToastItem-shaped object. */
export function pushToast(
  message: string | Partial<Omit<ToastItem, 'id'>>,
  kind: ToastKind = 'info'
) {
  const id = nextId++
  const item: ToastItem =
    typeof message === 'string'
      ? { id, message, kind }
      : { id, message: message.message ?? '', kind: message.kind ?? kind, ...message }
  toasts.push(item)
  // NOTE: auto-dismiss is owned by Toast.svelte (it needs to play toastOut
  // before removal) — the store only records the desired lifetime.
}

export function dismissToast(id: number) {
  toasts.splice(
    toasts.findIndex((t) => t.id === id),
    1
  )
}
