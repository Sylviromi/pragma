/// App-level UI state shared across components (tab bar toggle → sidebar).
export const ui = $state({
  sidebarOpen: true,
  railOpen: true,
  /** Right-sidebar panel tab: 'plan' | 'agents' | 'changes' (design v2). */
  panelTab: 'plan' as 'plan' | 'agents' | 'changes',
});

export function setPanelTab(t: 'plan' | 'agents' | 'changes') {
  ui.panelTab = t;
}
export function toggleSidebar() {
  ui.sidebarOpen = !ui.sidebarOpen;
}

export function toggleRail() {
  ui.railOpen = !ui.railOpen;
}
