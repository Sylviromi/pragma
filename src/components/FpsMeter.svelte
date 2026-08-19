<script lang="ts">
  // Debug performance overlay — toggle with Ctrl+Shift+F (persisted).
  //
  // Read it like a dashboard:
  //  - top: fps + a status word (smooth / busy / janky) + the cadence (mode
  //    frame time — the engine's typical rAF period).
  //  - frame: p50 / p95 / max frame times and dropped frames (>2× cadence).
  //  - long task: main-thread blocks >50ms this second (Long Task API).
  //  - input delay: worst event dispatch → handler-start gap (Event Timing) —
  //    the number to watch for "slow while typing".
  //  - nodes: DOM size. cv / longtask / evtiming: engine capability badges.
  //  - sparkline: last ~60 frame times (green ≈ cadence, yellow >1.5×, red >3×).
  //
  // WebKitGTK note: its rAF is not strictly vsync-locked, so absolute fps
  // wobbles; trust the tail (p95/max/dropped/long/input) over the fps number.
  import { onMount } from 'svelte';
  import { dev } from '$app/environment';

  const LS_KEY = 'pragma:fps';

  let show = $state(false);
  let stats = $state({
    fps: 0,
    cad: 0,
    cadHz: 0,
    p50: 0,
    p95: 0,
    max: 0,
    dropped: 0,
    long: 0,
    longMax: 0,
    evCount: 0,
    evDelay: 0,
    evDur: 0,
    status: '–' as string,
  });
  let nodes = $state(0);
  let supportCv = $state(false);
  let supportLong = $state(false);
  let supportEv = $state(false);
  let spark = $state<number[]>([]);

  let longCount = 0;
  let longMax = 0;
  let evCount = 0;
  let evDelay = 0;
  let evDur = 0;

  onMount(() => {
    // Debug-only component: if it's ever mounted in a production build the
    // whole thing stays inert (no observers, no listeners, no frame loop).
    if (!dev) return;
    // Auto-show via ?fps=1 URL param (browser test) or the persisted toggle.
    try {
      show =
        new URLSearchParams(window.location.search).get('fps') === '1' ||
        localStorage.getItem(LS_KEY) === '1';
    } catch {
      /* storage unavailable */
    }
    supportCv =
      typeof CSS !== 'undefined' && typeof CSS.supports === 'function'
        ? CSS.supports('content-visibility', 'auto')
        : false;

    // Long tasks (main-thread blocks > 50ms).
    let longObs: PerformanceObserver | null = null;
    try {
      longObs = new PerformanceObserver((list) => {
        for (const e of list.getEntries()) {
          longCount += 1;
          if (e.duration > longMax) longMax = e.duration;
        }
      });
      longObs.observe({ entryTypes: ['longtask'] });
      supportLong = true;
    } catch {
      /* engine without longtask */
    }

    // Interaction timing: dispatch → handler-start delay + total duration.
    let evObs: PerformanceObserver | null = null;
    try {
      evObs = new PerformanceObserver((list) => {
        for (const e of list.getEntries()) {
          const entry = e as PerformanceEventTiming & { interactionId?: number };
          // Skip synthetic/retargeted noise: only real interactions.
          if ((entry.interactionId ?? 0) === 0) continue;
          const delay = entry.processingStart - entry.startTime;
          if (delay > evDelay) evDelay = delay;
          if (entry.duration > evDur) evDur = entry.duration;
          evCount += 1;
        }
      });
      evObs.observe({ type: 'event' });
      supportEv = true;
    } catch {
      /* engine without event timing */
    }

    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.shiftKey && e.key.toLowerCase() === 'f') {
        e.preventDefault();
        show = !show;
        try {
          localStorage.setItem(LS_KEY, show ? '1' : '0');
        } catch {
          /* ignore */
        }
      }
    };
    window.addEventListener('keydown', onKey);
    return () => {
      window.removeEventListener('keydown', onKey);
      longObs?.disconnect();
      evObs?.disconnect();
    };
  });

  // Frame-time ring: last 120 rAF deltas. Cadence = most common frame time
  // (mode) — robust where rAF isn't vsync-locked.
  const ring: number[] = [];
  let prevFps = 0;
  let last = 0;
  let raf = 0;
  let iv: ReturnType<typeof setInterval> | null = null;

  $effect(() => {
    if (!show) return;
    last = performance.now();
    const loop = (t: number) => {
      const dt = t - last;
      last = t;
      // Skip the first gap and >500ms pauses (window switches, long GC).
      if (dt > 0 && dt < 500) {
        ring.push(dt);
        if (ring.length > 120) ring.shift();
      }
      raf = requestAnimationFrame(loop);
    };
    raf = requestAnimationFrame(loop);
    iv = setInterval(() => {
      const n = ring.length;
      if (n > 0) {
        const sorted = [...ring].sort((a, b) => a - b);
        const p50 = sorted[Math.floor(n * 0.5)];
        const p95 = sorted[Math.floor(n * 0.95)];
        const max = sorted[n - 1];
        const freq = new Map<number, number>();
        for (const d of ring) {
          const ms = Math.round(d);
          freq.set(ms, (freq.get(ms) ?? 0) + 1);
        }
        let cad = p50;
        let best = -1;
        for (const [ms, c] of freq) {
          if (c > best) {
            best = c;
            cad = ms;
          }
        }
        const newFps = Math.round(1000 / p50);
        const fps = prevFps ? Math.round(prevFps * 0.4 + newFps * 0.6) : newFps;
        prevFps = fps;
        const dropped = sorted.filter((d) => d > 33).length;
        const status =
          dropped > 0 || longCount > 0 || max > 100
            ? 'janky'
            : p95 > 33 || evDelay > 16
              ? 'busy'
              : 'smooth';
        stats = {
          fps,
          cad,
          cadHz: Math.round(1000 / Math.max(cad, 1)),
          p50: Math.round(p50),
          p95: Math.round(p95),
          max: Math.round(max),
          dropped,
          long: longCount,
          longMax: Math.round(longMax),
          evCount,
          evDelay: Math.round(evDelay),
          evDur: Math.round(evDur),
          status,
        };
      }
      longCount = 0;
      longMax = 0;
      evCount = 0;
      evDelay = 0;
      evDur = 0;
      spark = ring.slice(-60).map((d) => Math.min(28, Math.round(d)));
      nodes = document.querySelectorAll('*').length;
    }, 1000);
    return () => {
      cancelAnimationFrame(raf);
      if (iv) {
        clearInterval(iv);
        iv = null;
      }
    };
  });
</script>

{#if show}
  <div class="fps" aria-hidden="true">
    <div class="head">
      <span class="fps-num" class:busy={stats.status === 'busy'} class:bad={stats.status === 'janky'}>
        {stats.fps}
      </span>
      <span class="head-side">
        <span class="status" class:busy={stats.status === 'busy'} class:bad={stats.status === 'janky'}>
          {stats.status}
        </span>
        <span class="cad">cad {stats.cad}ms ≈ {stats.cadHz}hz</span>
      </span>
    </div>

    <div class="grid">
      <div class="cell">
        <span class="lbl">p50</span>
        <span class="val">{stats.p50}ms</span>
      </div>
      <div class="cell">
        <span class="lbl">p95</span>
        <span class="val">{stats.p95}ms</span>
      </div>
      <div class="cell">
        <span class="lbl">max</span>
        <span class="val" class:y={stats.max > 50} class:bad={stats.max > 100}>{stats.max}ms</span>
      </div>
      <div class="cell">
        <span class="lbl">dropped</span>
        <span class="val" class:bad={stats.dropped > 0}>{stats.dropped}</span>
      </div>
      <div class="cell">
        <span class="lbl">long task</span>
        <span class="val" class:bad={stats.long > 0}>{supportLong ? stats.long : '–'}</span>
      </div>
      <div class="cell">
        <span class="lbl">input delay</span>
        <span class="val" class:y={stats.evDelay > 16} class:bad={stats.evDelay > 50}>
          {supportEv ? `${stats.evDelay}ms` : '–'}
        </span>
      </div>
    </div>

    <div class="caps">
      <span class="cap" class:on={supportCv}>cv</span>
      <span class="cap" class:on={supportLong}>longtask</span>
      <span class="cap" class:on={supportEv}>evtiming</span>
      <span class="nodes">{nodes} nodes</span>
    </div>

    <div class="spark">
      {#each spark as v, i (i)}
        <span
          class="bar"
          class:y={v > 16}
          class:r={v > 33}
          style={`height:${v}px`}
        ></span>
      {/each}
    </div>
    <span class="hint">ctrl+shift+f hides</span>
  </div>
{/if}

<style>
  .fps {
    position: fixed;
    top: 48px;
    left: 12px;
    z-index: 9999;
    display: flex;
    flex-direction: column;
    gap: 7px;
    padding: 8px 10px;
    background: oklch(12% 0.02 250 / 0.92);
    border: 1px solid var(--border-default);
    border-radius: 5px;
    font-family: var(--font-mono);
    pointer-events: none;
    user-select: none;
    text-align: left;
    width: 168px;
  }

  /* Head: fps number + status chip + cadence. */
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .fps-num {
    font-size: 26px;
    font-weight: 600;
    line-height: 1;
    color: var(--success);
  }
  .fps-num.busy {
    color: var(--warning);
  }
  .fps-num.bad {
    color: var(--danger);
  }
  .head-side {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 2px;
  }
  .status {
    font-size: 9px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    padding: 1px 6px;
    border-radius: 2px;
    color: var(--success);
    background: color-mix(in oklch, var(--success) 14%, transparent);
  }
  .status.busy {
    color: var(--warning);
    background: color-mix(in oklch, var(--warning) 14%, transparent);
  }
  .status.bad {
    color: var(--danger);
    background: color-mix(in oklch, var(--danger) 14%, transparent);
  }
  .cad {
    font-size: 9px;
    color: var(--text-muted);
  }

  /* Labeled value grid. */
  .grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 5px 8px;
    border-top: 1px solid var(--border-subtle);
    padding-top: 7px;
  }
  .cell {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .lbl {
    font-size: 8px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--text-muted);
  }
  .val {
    font-size: 12.5px;
    color: var(--text-primary);
    line-height: 1.2;
  }
  .val.y {
    color: var(--warning);
  }
  .val.bad {
    color: var(--danger);
  }

  /* Engine capability badges + node count. */
  .caps {
    display: flex;
    align-items: center;
    gap: 4px;
    border-top: 1px solid var(--border-subtle);
    padding-top: 6px;
  }
  .cap {
    font-size: 8px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    padding: 1px 5px;
    border-radius: 2px;
    color: var(--text-muted);
    background: var(--bg-overlay);
  }
  .cap.on {
    color: var(--success);
    background: color-mix(in oklch, var(--success) 14%, transparent);
  }
  .cap:not(.on)::after {
    content: ' ✗';
  }
  .nodes {
    margin-left: auto;
    font-size: 9px;
    color: var(--text-muted);
  }

  /* Frame-time sparkline: last ~60 frames (green ≈ cadence, yellow >1.5×,
     red >3× = jank). */
  .spark {
    display: flex;
    align-items: flex-end;
    gap: 1px;
    height: 28px;
    border-top: 1px solid var(--border-subtle);
    padding-top: 6px;
  }
  .bar {
    width: 2px;
    background: var(--success);
  }
  .bar.y {
    background: var(--warning);
  }
  .bar.r {
    background: var(--danger);
  }

  .hint {
    font-size: 8px;
    color: var(--text-muted);
    opacity: 0.75;
  }
</style>
