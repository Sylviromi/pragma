<script lang="ts">
  let theme = $state(document.documentElement.getAttribute('data-theme') ?? 'dark')

  function setTheme(t: string) {
    theme = t
    document.documentElement.setAttribute('data-theme', t)
    localStorage.setItem('pragma:theme', t)
  }
</script>

<h2 style="margin-top:0;color:var(--color-text-primary)">Appearance</h2>
<p style="color:var(--color-text-muted);font-size:var(--font-size-sm)">Choose a theme.</p>

<div style="display:flex;gap:var(--spacing-3);margin-top:var(--spacing-4)">
  {#each ['dark', 'light', 'high-contrast'] as t}
    <button
      onclick={() => setTheme(t)}
      style="
      padding:var(--spacing-3) var(--spacing-4);border-radius:var(--radius-md);cursor:pointer;
      border: 2px solid {theme === t ? 'var(--color-accent-brand)' : 'var(--color-border-default)'};
      background:{theme === t ? 'color-mix(in srgb, var(--color-accent-brand) 10%, transparent)' : 'var(--color-bg-elevated)'};
      color:var(--color-text-primary);font-size:var(--font-size-sm);text-transform:capitalize"
    >
      {t.replace('-', ' ')}
    </button>
  {/each}
</div>