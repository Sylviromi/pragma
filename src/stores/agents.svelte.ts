import type { AgentStatus } from './chat.svelte'

/// Live sub-agents + detached processes for the sidebar's Agents pane.
///
/// TODO(agents): backend wiring — the agentic loop currently has no
/// sub-agent spawning and no detached-process management, so this store stays
/// empty and the Agents pane renders its upcoming empty state. When the
/// backend lands (see TODO.md), wire the events here:
///   - `agent:spawned` / `agent:step` / `agent:done` → tree nodes + steps
///   - `process:spawned` / `process:log` / `process:exit` → nodes + log lines
///   - keep `selectedId` + `follow` as the console's view state.

export type AgentNodeState =
  | 'running'
  | 'thinking'
  | 'detached'
  | 'done'
  | 'exited'
  | 'failed'

export interface AgentNode {
  id: string
  kind: 'agent' | 'process'
  name: string
  meta: string
  state: AgentNodeState
  /** Tree depth for indent (sub-agents of sub-agents). */
  depth: number
}

export const agents = $state({
  nodes: [] as AgentNode[],
  /** Process/agent id → log lines (stdout-ish text). */
  logs: {} as Record<string, string[]>,
  selectedId: null as string | null,
  follow: true,
})

export function selectNode(id: string) {
  agents.selectedId = id
}

export function toggleFollow() {
  agents.follow = !agents.follow
}

/// Convenience summary for the pane header badge: "n live · m proc".
export function agentsSummary(): string {
  const live = agents.nodes.filter((n) => n.kind === 'agent' && n.state === 'running').length
  const procs = agents.nodes.filter(
    (n) => n.kind === 'process' && (n.state === 'running' || n.state === 'detached')
  ).length
  return `${live} live · ${procs} proc`
}
