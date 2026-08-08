<script lang="ts">
  import { readPluginComponent } from '../lib/tauri'
  import { cacheKeyFor } from '../stores/plugins.svelte'

  let { pluginId, slot, context = '' }: { pluginId: string; slot: string; context?: string } = $props()

  let host = $state<HTMLElement | null>(null)

  // Module-level caches (keyed by plugin id) so we don't re-read the file on every mount.
  const codeCache = new Map<string, { code: string; version: number }>()
  const elementRegistry = new Set<string>()

  function sanitizeId(id: string): string {
    return 'pragma-plugin-' + id.replace(/[^a-z0-9-]/g, '-')
  }

  $effect(() => {
    // Re-run whenever the plugin, slot, context, or cache key changes.
    void cacheKeyFor(pluginId)
    void slot
    void context
    let cancelled = false

    async function mount() {
      if (!host || !pluginId) return
      const key = cacheKeyFor(pluginId)
      // Read code (fresh if the cache key moved on)
      if (codeCache.get(pluginId)?.version !== key) {
        try {
          const code = await readPluginComponent(pluginId)
          codeCache.set(pluginId, { code, version: key })
        } catch (err) {
          console.warn('[plugin] cannot load component for', pluginId, err)
          return
        }
      }
      const entry = codeCache.get(pluginId)
      if (!entry || cancelled) return

      const name = sanitizeId(pluginId)
      try {
        if (!elementRegistry.has(name) && !customElements.get(name)) {
          const blobUrl = URL.createObjectURL(
            new Blob([entry.code], { type: 'text/javascript' })
          )
          try {
            const mod = await import(/* @vite-ignore */ blobUrl)
            const Ctor = (mod as { default?: typeof HTMLElement }).default
            if (typeof Ctor !== 'function') {
              console.warn('[plugin]', pluginId, 'component.js must export a class (default export)')
              return
            }
            customElements.define(name, Ctor)
            elementRegistry.add(name)
          } finally {
            URL.revokeObjectURL(blobUrl)
          }
        }
      } catch (err) {
        console.warn('[plugin] failed to define element for', pluginId, err)
        return
      }

      // Mount an instance into the host.
      const el = document.createElement(name)
      el.setAttribute('data-slot', slot)
      if (context) el.setAttribute('data-context', context)
      host.replaceChildren(el)
    }

    mount()
    return () => {
      cancelled = true
      host?.replaceChildren()
    }
  })
</script>

<div class="plugin-slot-host" bind:this={host} data-plugin={pluginId} data-slot={slot}></div>

<style>
  .plugin-slot-host {
    display: contents;
  }
</style>