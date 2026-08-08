export type ToastKind = 'info' | 'success' | 'error'

export interface ToastItem {
  id: number
  message: string
  kind: ToastKind
}

export const toasts = $state<ToastItem[]>([])

let nextId = 1

export function pushToast(message: string, kind: ToastKind = 'info') {
  const id = nextId++
  toasts.push({ id, message, kind })
  setTimeout(() => dismissToast(id), 3200)
}

export function dismissToast(id: number) {
  toasts.splice(
    toasts.findIndex((t) => t.id === id),
    1
  )
}