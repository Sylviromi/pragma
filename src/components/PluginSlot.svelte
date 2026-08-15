<script lang="ts">
  import { readPluginComponent } from '../lib/tauri'
  import { cacheKeyFor } from '../stores/plugins.svelte'

  let { pluginId, slot, context = '' }: { pluginId: string; slot: string; context?: string } = $props()
  import { pushToast } from '../stores/toast.svelte'

  let host = $state<HTMLElement | null>(null)

  // Module-level cache (keyed by plugin id) so we don't re-read the file on
  // every mount. The version tracks the store's cache key — a moved key
  // means the code changed (plugin hot reload) and we must import it fresh.
  const codeCache = new Map<string, { code: string; version: number }>()

  /** Import a plugin's component code and return its default-export class.
   *  Always imports a fresh module for the given code (blob URL is unique per
   *  read), so hot-reloaded code actually replaces the previous class — a
   *  plain `customElements.define` would keep reusing the FIRST class ever
   *  loaded (the registry never redefines). */
  async function loadClass(code: string): Promise<typeof HTMLElement | null> {
    const blobUrl = URL.createObjectURL(new Blob([code], { type: 'text/javascript' }))
    try {
      const mod = await import(/* @vite-ignore */ blobUrl)
      const Ctor = (mod as { default?: typeof HTMLElement }).default
      return typeof Ctor === 'function' ? Ctor : null
    } finally {
      URL.revokeObjectURL(blobUrl)
    }
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
          const msg = `[${pluginId}] cannot read component: ${err}`
          console.warn(msg)
          pushToast(msg, 'error')
          return
        }
      }
      const entry = codeCache.get(pluginId)
      if (!entry || cancelled) return

      try {
        const Ctor = await loadClass(entry.code)
        if (!Ctor) {
          const msg = `[${pluginId}] component.js must export a class`
          console.warn(msg)
          pushToast(msg, 'error')
          return
        }
        // Instantiate directly — no global registry, so every mount (incl.
        // hot reload) uses the CURRENT code.
        const el = new Ctor()
        el.setAttribute('data-slot', slot)
        if (context) el.setAttribute('data-context', context)
        host.replaceChildren(el)
      } catch (err) {
        console.warn('[plugin] failed to load component for', pluginId, err)
      }
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