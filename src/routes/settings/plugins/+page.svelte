<h2 style="margin-top:0;color:var(--color-text-primary)">Plugins</h2>
<p class="desc">Drop plugin folders into <code>~/.config/pragma/plugins/</code> — they hot-reload instantly.</p>

<div class="empty">
  <svg width="32" height="32" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.2" aria-hidden="true">
    <rect x="2" y="2" width="6.5" height="6.5"/>
    <rect x="7.5" y="7.5" width="6.5" height="6.5"/>
  </svg>
  <p>No plugins installed yet.</p>
  <p>Plugin support is coming in the next release.</p>
</div>

<style>
  h2 { font-size: var(--font-size-lg); font-weight: 600; margin-bottom: var(--spacing-2); }
  .desc { font-size: var(--font-size-sm); color: var(--color-text-muted); margin: 0 0 var(--spacing-6); }
  code {
    background: var(--color-bg-overlay); padding: 1px 5px;
    border-radius: var(--radius-sm); font-family: var(--font-mono); font-size: var(--font-size-xs);
  }
  .empty {
    display: flex; flex-direction: column; align-items: center; justify-content: center;
    gap: var(--spacing-2); padding: var(--spacing-6) 0;
    color: var(--color-text-muted); font-size: var(--font-size-sm);
  }
  .empty p { margin: 0; }
  .empty svg { opacity: 0.4; margin-bottom: var(--spacing-2); }
</style>
