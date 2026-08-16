import type { SessionStats } from '../lib/types';
import { getSessionStats } from '../lib/tauri';
import { isBrowserMock, mockStats } from '../lib/mock';
import { listen } from '@tauri-apps/api/event';

/// Empty stats — rendered sections show placeholders until the backend
/// telemetry lands (get_session_stats / `session:stats` events).
function emptyStats(sessionId: string): SessionStats {
  return {
    sessionId,
    context: {
      segments: [
        { label: 'System + skills', tokens: 0, color: 'var(--text-muted)' },
        { label: 'Files in context', tokens: 0, color: 'var(--warning)' },
        { label: 'Conversation', tokens: 0, color: 'var(--accent)' },
      ],
      used: 0,
      limit: 200000,
      percent: 0,
    },
    cost: { rows: [], total: '$0.00' },
    plan: { items: [], progress: '' },
    files: [],
    git: { branch: '', added: 0, removed: 0, files: 0, rows: [] },
  };
}

export const stats = $state<Record<string, SessionStats>>({});

export function statsFor(sessionId: string): SessionStats {
  return stats[sessionId] ?? emptyStats(sessionId);
}

/** True once a payload has been fetched or pushed for this session. Lets
 *  the sidebar show what it ALREADY has (last push from the run loop or the
 *  one load) instead of re-fetching telemetry every time the panel opens. */
export function hasSessionStats(sessionId: string): boolean {
  return stats[sessionId] !== undefined;
}

function setStats(sessionId: string, s: SessionStats) {
  stats[sessionId] = s;
}

/** Load stats for a session. Falls back to mock data in the plain-browser
 *  dev mode. Called lazily the first time the sidebar needs a session (a
 *  session that has already been pushed via `session:stats` stays as-is);
 *  live pushes keep it fresh while a run is active. */
export async function loadSessionStats(sessionId: string): Promise<void> {
  try {
    const s = isBrowserMock ? (mockStats[sessionId] ?? null) : await getSessionStats(sessionId);
    if (s) setStats(sessionId, s);
  } catch {
    // Backend telemetry not available yet — keep the previous/empty state.
  }
}

/// Subscribe to `session:stats` pushes from the agentic loop so the sidebar
/// updates live mid-run. No-op in the plain-browser mock (no backend events).
export async function initStatsEvents(): Promise<void> {
  if (isBrowserMock) return;
  try {
    await listen<SessionStats>('session:stats', (e) => {
      if (e.payload?.sessionId) setStats(e.payload.sessionId, e.payload);
    });
    // Per-session pushes from the loop (`session:stats:{id}` glob).
    await listen<SessionStats>('session:stats:*', (e) => {
      if (e.payload?.sessionId) setStats(e.payload.sessionId, e.payload);
    });
  } catch {
    // events bus unavailable — polling via loadSessionStats still works
  }
}
