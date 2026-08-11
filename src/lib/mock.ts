// Browser-only mock data — used when the frontend runs without the Tauri
// backend (plain `vite dev` in a browser, design iteration, plugin dev).
// Guarded by tauri.ts: never active inside the real app.
import type { Message, ProviderInfo, Session, PluginInfo, SessionStats, SkillInfo } from './types'

export const isBrowserMock =
  typeof window !== 'undefined' && !('__TAURI_INTERNALS__' in (window as unknown as Record<string, unknown>))

const now = (m = 0) => new Date(Date.now() - m * 60000).toISOString()

export const mockSessions: Session[] = [
  {
    id: 'mock-my-project', name: 'my-project', workingDir: '/mnt/Projects/pragma',
    createdAt: now(240), updatedAt: now(5), model: 'claude-sonnet-4-6', provider: 'anthropic',
    archived: false, metadataJson: '{}',
  },
  {
    id: 'mock-rust-game', name: 'rust-game', workingDir: '/mnt/Projects/game',
    createdAt: now(180), updatedAt: now(40), model: 'claude-opus-4-6', provider: 'anthropic',
    archived: false, metadataJson: '{}',
  },
]

export const mockMessages: Record<string, Message[]> = {
  'mock-my-project': [
    {
      id: 'm1', sessionId: 'mock-my-project', role: 'user',
      content: JSON.stringify('Why is the dev server crashing on hot reload?'),
      model: '', provider: '', timestamp: now(30), tokens: 0,
    },
    {
      id: 'm2', sessionId: 'mock-my-project', role: 'assistant',
      content: JSON.stringify([
        { type: 'reasoning', content: 'A crash that only happens on reload smells like a listener that outlives its module. Let me read the hot-reload hook.' },
        { type: 'tool_use', id: 'tu1', name: 'read_file', input: { path: 'src/hooks/useHotReload.ts' } },
        { type: 'text', text: 'Stale WebSocket handler in useHotReload.ts — the listener isn\'t cleaned up between reloads, so the second connection throws. Add a cleanup in the effect return:\n```ts\nuseEffect(() => {\n  const ws = connect();\n  return () => ws.close();\n}, []);\n```' },
      ]),
      model: 'claude-sonnet-4-6', provider: 'anthropic', timestamp: now(28), tokens: 428,
    },
    {
      id: 'm3', sessionId: 'mock-my-project', role: 'user',
      content: JSON.stringify([{ type: 'tool_result', tool_use_id: 'tu1', content: 'export function useHotReload() {\n  const ws = new WebSocket(HMR_URL);\n  ws.addEventListener("message", applyUpdate);\n  // no cleanup on module dispose\n}' }]),
      model: '', provider: '', timestamp: now(27), tokens: 0,
    },
    {
      id: 'm4', sessionId: 'mock-my-project', role: 'assistant',
      content: JSON.stringify([
        { type: 'text', text: 'Fixed it. The effect now returns the cleanup so the socket closes on every HMR cycle. The dev server should stop crashing on reload — restart it and it\'ll stay up.' },
      ]),
      model: 'claude-sonnet-4-6', provider: 'anthropic', timestamp: now(25), tokens: 96,
    },
  ],
  'mock-rust-game': [
    {
      id: 'g1', sessionId: 'mock-rust-game', role: 'user',
      content: JSON.stringify('Set up a basic ECS for the physics system'),
      model: '', provider: '', timestamp: now(50), tokens: 0,
    },
    {
      id: 'g2', sessionId: 'mock-rust-game', role: 'assistant',
      content: JSON.stringify([
        { type: 'reasoning', content: 'Cache locality matters more than ergonomics here, so sparse sets over maps.' },
        { type: 'tool_use', id: 'gu1', name: 'read_file', input: { path: 'src/physics/entity.rs' } },
        { type: 'text', text: 'Minimal entity-component setup using sparse sets — components live in flat arrays keyed by a dense entity index, so iteration stays branch-free.' },
      ]),
      model: 'claude-opus-4-6', provider: 'anthropic', timestamp: now(45), tokens: 612,
    },
  ],
}

export const mockProviders: ProviderInfo[] = [
  {
    id: 'anthropic', name: 'Anthropic', source: 'builtin',
    models: [
      { id: 'claude-sonnet-4-6', name: 'claude-sonnet-4-6', contextWindow: 200000, isFree: false, isVision: false, supportsThinking: true, supportsThinkingLevels: true },
      { id: 'claude-opus-4-6', name: 'claude-opus-4-6', contextWindow: 200000, isFree: false, isVision: false, supportsThinking: true, supportsThinkingLevels: true },
    ],
  },
  {
    id: 'deepseek', name: 'DeepSeek', source: 'builtin',
    models: [{ id: 'deepseek-v4-flash', name: 'deepseek-v4-flash', contextWindow: 65536, isFree: false, isVision: false, supportsThinking: false, supportsThinkingLevels: false }],
  },
  {
    id: 'zen-warp', name: 'Zen WARP', source: { plugin: { pluginId: 'zen-warp' } },
    models: [
      { id: 'deepseek-v4 via warp', name: 'deepseek-v4 via warp', contextWindow: 200000, isFree: true, isVision: false, supportsThinking: false, supportsThinkingLevels: false },
      { id: 'mimo-v2.5 via warp', name: 'mimo-v2.5 via warp', contextWindow: 128000, isFree: true, isVision: true, supportsThinking: false, supportsThinkingLevels: false },
    ],
  },
]

export const mockPlugins: PluginInfo[] = [
  {
    id: 'zen-warp', name: 'Zen WARP', version: '2.4.1', description: 'Free and low-cost model routing through the WARP relay.',
    enabled: true, folder: 'zen-warp', hasComponent: false, hasSkill: false, skills: [],
    contributes: { providers: ['zen-warp'], slots: [], commands: [], skills: [] },
  },
  {
    id: 'handsfree', name: 'Handsfree', version: '1.1.0', description: 'Autonomous multi-step execution with phase reporting.',
    enabled: true, folder: 'handsfree', hasComponent: true, hasSkill: false, skills: [],
    contributes: { providers: [], slots: ['status-bar'], commands: [], skills: [] },
  },
]

export const mockSkills: SkillInfo[] = [
  {
    id: 'git', name: 'git', source: 'native', pluginId: null,
    description: 'Version control: git status, diff, log, add, commit', hasTools: true, hasHandler: false,
  },
  {
    id: 'web', name: 'web', source: 'native', pluginId: null,
    description: 'Web search + fetch for external documentation', hasTools: true, hasHandler: false,
  },
  {
    id: 'frontend-dev/frontend-dev', name: 'frontend-dev', source: 'plugin-skill', pluginId: 'frontend-dev',
    description: 'Frontend Developer skill: live localhost server with tool control', hasTools: true, hasHandler: true,
  },
]

export const mockStats: Record<string, SessionStats> = {
  'mock-my-project': {
    sessionId: 'mock-my-project',
    context: {
      segments: [
        { label: 'System + skills', tokens: 7400, color: 'var(--text-muted)' },
        { label: 'Files in context', tokens: 31200, color: 'var(--warning)' },
        { label: 'Conversation', tokens: 18600, color: 'var(--accent)' },
      ],
      used: 57200, limit: 200000, percent: 29,
    },
    cost: {
      rows: [
        { model: 'claude-sonnet-4-6', tokens: '52.4k', cost: '$0.41' },
        { model: 'claude-opus-4-6', tokens: '11.8k', cost: '$0.62' },
        { model: 'deepseek-v4 (warp)', tokens: '9.1k', cost: '$0.00' },
      ],
      total: '$1.03',
    },
    plan: {
      items: [
        { label: 'Reproduce the crash on a cold start', state: 'done' },
        { label: 'Trace the selector back to the store slice', state: 'done' },
        { label: 'Guard hydration in the reducer', state: 'active' },
        { label: 'Add a regression test', state: 'todo' },
        { label: 'Run the full suite', state: 'todo' },
      ],
      progress: '2/5',
    },
    files: [
      { path: 'src/store/reducer.ts', kind: 'edited', added: 24, removed: 12 },
      { path: 'src/hooks/useHotReload.ts', kind: 'edited', added: 6, removed: 3 },
      { path: 'src/store/selectors.ts', kind: 'read' },
      { path: 'tests/reducer.hydration.spec.ts', kind: 'created', added: 41, removed: 0 },
      { path: 'src/physics/entity.rs', kind: 'read' },
    ],
    git: {
      branch: 'fix/hydration', added: 71, removed: 15, files: 3,
      rows: [
        { mark: 'M', path: 'src/store/reducer.ts', weight: 1 },
        { mark: 'M', path: 'src/hooks/useHotReload.ts', weight: 0.35 },
        { mark: 'A', path: 'tests/reducer.hydration.spec.ts', weight: 0.8 },
      ],
    },
  },
}
