<script lang="ts">
  import PluginSlot from './PluginSlot.svelte'
  import { plugins } from '../stores/plugins.svelte'

  // Slots the vanilla layout places explicitly (anchored). Any slot a plugin
  // claims OUTSIDE this set is free-floating: the app mounts the plugin in a
  // global overlay layer and the plugin positions itself (position: fixed)
  // wherever it wants — status strips, preview panels, pets that walk across
  // the screen, anything. A plugin can take whatever UI space it wants by
  // claiming a new slot name; no vanilla code change is needed.
  const ANCHORED_SLOTS = new Set([
    'input-toolbar',
    'chat-toolbar-extra',
    'composer-center',
    'model-picker-extra',
    'status-bar',
    'sidebar-section',
    'chat-view',
    'settings-page',
  ])

  const free = $derived(
    plugins.list.filter(
      (p) =>
        p.enabled &&
        p.hasComponent &&
        p.contributes.slots.some((s) => !ANCHORED_SLOTS.has(s))
    )
  )

  // Each free plugin mounts once, tagged with its first unanchored slot so
  // the component can tell which surface it's occupying.
  const slotFor = (p: (typeof plugins.list)[number]): string =>
    p.contributes.slots.find((s) => !ANCHORED_SLOTS.has(s)) ?? p.contributes.slots[0] ?? 'free'
</script>

{#if free.length}
  <div class="plugin-free-layer">
    {#each free as p (p.id)}
      <PluginSlot slot={slotFor(p)} pluginId={p.id} />
    {/each}
  </div>
{/if}

<style>
  /* Global free-positioning surface: fully transparent and non-interactive
     itself — plugins re-enable pointer events on their own :host. Anchored
     to the (scaled) app via inset:0, so fixed children inside it position
     relative to the app's coordinate space (see skill.md: divide exact
     cursor coordinates by --ui-scale). */
  .plugin-free-layer {
    position: fixed;
    inset: 0;
    z-index: 900; /* below toasts/modals (1000) */
    pointer-events: none;
  }
</style>
