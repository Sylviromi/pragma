/// App-level UI state shared across components (tab bar toggle → sidebar).
export const ui = $state({
  sidebarOpen: true,
})

export function toggleSidebar() {
  ui.sidebarOpen = !ui.sidebarOpen
}