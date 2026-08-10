<script lang="ts">
  import PluginSlot from './PluginSlot.svelte'
  import { plugins, pluginsForSlot } from '../stores/plugins.svelte'

  let { slot, context = '' }: { slot: string; context?: string } = $props()

  // settings-page is a per-plugin slot: only the plugin whose settings page is
  // being viewed renders there. Every other slot stacks all claiming plugins
  // (multiple status strips, multiple toolbar buttons, …).
  const apps = $derived(
    slot === 'settings-page'
      ? pluginsForSlot(slot).filter((p) => p.id === context)
      : pluginsForSlot(slot)
  )
</script>

{#if apps.length > 0}
  <div class="slot-stack" data-slot={slot}>
    {#each apps as p (p.id)}
      <PluginSlot {slot} {context} pluginId={p.id} />
    {/each}
  </div>
{/if}

<style>
  .slot-stack {
    display: contents;
  }
  .slot-stack:empty {
    display: none;
  }
</style>