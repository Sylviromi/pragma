<script lang="ts">
  import { toasts, dismissToast } from '../stores/toast.svelte'
  import Icon from './Icon.svelte'

  // Design toast (`Pragma UI Kit v2.dc.html` → `toasts`): 340px elevated card,
  // 3px colored rail, status icon, title + mono source, optional desc + action
  // links, dismiss button and a bottom lifetime timer bar. Stack bottom-right.
  // Entrance = toastIn; dismissal (auto or manual) plays toastOut before the
  // store removes the item.
  const META: Record<string, [string, string]> = {
    success: ['task_alt', 'var(--success)'],
    error: ['error', 'var(--danger)'],
    warning: ['warning', 'var(--warning)'],
    info: ['info', 'var(--accent)'],
    progress: ['sync', 'var(--slot)'],
  }

  // Ids currently playing their exit animation (toastOut) — keep mounted.
  const exiting = $state<Set<number>>(new Set())
  const timers = new Map<number, ReturnType<typeof setTimeout>>()

  function scheduleAutoDismiss(t: { id: number; life?: number }) {
    const life = t.life ?? 3200
    if (life <= 0) return
    const timer = setTimeout(() => animateOut(t.id), life)
    timers.set(t.id, timer)
  }

  function animateOut(id: number) {
    if (exiting.has(id)) return
    const timer = timers.get(id)
    if (timer) {
      clearTimeout(timer)
      timers.delete(id)
    }
    exiting.add(id)
    setTimeout(() => {
      exiting.delete(id)
      dismissToast(id)
    }, 240)
  }

  function dismiss(id: number) {
    // Instant removal on click (still polite: short exit).
    animateOut(id)
  }

  $effect(() => {
    // Start lifetime timers for newly-pushed toasts.
    for (const t of toasts) {
      if (!timers.has(t.id)) scheduleAutoDismiss(t)
    }
  })
</script>

<div class="toast-stack" role="status" aria-live="polite">
  {#each toasts as t (t.id)}
    {@const [icon, color] = META[t.kind] ?? META.info}
    <div class="toast" class:exiting={exiting.has(t.id)} style={`--tcolor:${color}`} role="status">
      <span class="rail" aria-hidden="true"></span>
      <span class="t-ico" class:spin={t.kind === 'progress'} style={`color:${color}`} aria-hidden="true">
        <Icon name={icon as never} size={17} />
      </span>
      <div class="t-copy">
        <div class="t-title-row">
          <span class="t-title">{t.message}</span>
          {#if t.source}<span class="t-source">{t.source}</span>{/if}
        </div>
        {#if t.desc}<div class="t-desc">{t.desc}</div>{/if}
        {#if t.actions && t.actions.length > 0}
          <div class="t-actions">
            {#each t.actions as a (a.label)}
              <button
                class="t-action"
                onclick={() => {
                  animateOut(t.id)
                  a.onClick?.()
                }}
              >{a.label}</button>
            {/each}
          </div>
        {/if}
      </div>
      <button
        class="t-dismiss"
        aria-label="Dismiss"
        onclick={() => dismiss(t.id)}
      ><Icon name="close" size={15} /></button>
      {#if t.life && !exiting.has(t.id)}
        <span class="timer" style={`--life:${t.life}ms; --tcolor:${color}`} aria-hidden="true"></span>
      {/if}
    </div>
  {/each}
</div>

<style>
  /* Design: bottom-right stack, toastIn/toastOut, 340px cards. */
  .toast-stack {
    position: fixed;
    right: 18px;
    bottom: 18px;
    display: flex;
    flex-direction: column;
    gap: 9px;
    align-items: flex-end;
    pointer-events: none;
    max-width: min(400px, calc(100vw - 36px));
    z-index: 1000;
  }
  .toast {
    position: relative;
    pointer-events: auto;
    display: flex;
    align-items: flex-start;
    gap: 10px;
    width: 340px;
    max-width: 100%;
    padding: 11px 12px 12px 15px;
    background: var(--bg-elevated);
    border: 1px solid var(--border-default);
    box-shadow: var(--shadow-lg);
    overflow: hidden;
    animation: toastIn 0.3s cubic-bezier(0.22, 0.8, 0.3, 1) both;
  }
  .toast.exiting {
    animation: toastOut 0.24s cubic-bezier(0.22, 0.8, 0.3, 1) both;
  }
  .rail {
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 3px;
    background: var(--tcolor);
  }
  .t-ico {
    flex: none;
    display: flex;
    margin-top: 1px;
    color: var(--tcolor);
  }
  .t-ico :global(.msr) {
    font-size: 17px;
    line-height: 1;
  }
  .t-ico.spin {
    animation: spin 1.4s linear infinite;
  }
  .t-copy {
    flex: 1;
    min-width: 0;
  }
  .t-title-row {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }
  .t-title {
    flex: 1;
    min-width: 0;
    font-family: var(--font-body);
    font-size: 13px;
    font-weight: 500;
    color: var(--text-primary);
    text-wrap: pretty;
  }
  .t-source {
    flex: none;
    font-family: var(--font-mono);
    font-size: 9.5px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-muted);
  }
  .t-desc {
    font-family: var(--font-body);
    font-size: 12px;
    line-height: 1.45;
    color: var(--text-muted);
    margin-top: 3px;
    text-wrap: pretty;
  }
  .t-actions {
    display: flex;
    gap: 14px;
    margin-top: 8px;
  }
  .t-action {
    background: none;
    border: none;
    padding: 0;
    font-family: var(--font-mono);
    font-size: 10.5px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--accent);
    cursor: pointer;
  }
  .t-action:hover {
    color: var(--accent-hover);
  }
  .t-dismiss {
    flex: none;
    background: none;
    border: none;
    padding: 2px;
    color: var(--text-muted);
    cursor: pointer;
    display: flex;
  }
  .t-dismiss:hover {
    color: var(--text-primary);
  }
  .t-dismiss :global(.msr) {
    font-size: 15px;
    line-height: 1;
  }
  /* Lifetime bar: shrinks over the toast's lifetime (design toastTimer). */
  .timer {
    position: absolute;
    left: 0;
    bottom: 0;
    height: 2px;
    width: 100%;
    background: var(--tcolor);
    opacity: 0.5;
    transform-origin: left;
    animation: toastTimer var(--life) linear forwards;
  }

  @keyframes toastIn {
    from { opacity: 0; transform: translateY(12px) scale(0.97); }
    to { opacity: 1; transform: none; }
  }
  @keyframes toastOut {
    from { opacity: 1; transform: none; }
    to { opacity: 0; transform: translateY(10px) scale(0.97); }
  }
  @keyframes toastTimer {
    from { transform: scaleX(1); }
    to { transform: scaleX(0); }
  }
</style>
