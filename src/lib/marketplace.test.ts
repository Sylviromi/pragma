import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { mount, unmount } from 'svelte'
import Marketplace from '../routes/settings/marketplace/+page.svelte'
import { plugins } from '../stores/plugins.svelte'

// A fake registry index — one installable plugin, one already-installed id.
const INDEX = {
  plugins: [
    {
      id: 'alpha',
      name: 'Alpha',
      version: '1.0.0',
      description: 'A test plugin',
      author: 'Ada',
      publisher: '',
      source: 'git:https://github.com/test/repo@alpha-v1.0.0#plugins/alpha'
    },
    {
      id: 'zen-warp',
      name: 'Zen WARP',
      version: '2.4.1',
      description: 'already installed in mock data',
      author: '',
      publisher: '',
      source: 'git:https://github.com/test/repo@zen-warp-v2.4.1#plugins/zen-warp'
    }
  ]
}

beforeEach(() => {
  plugins.list = []
  // jsdom env here has no localStorage — provide an in-memory shim.
  const store = new Map<string, string>()
  vi.stubGlobal('localStorage', {
    getItem: (k: string) => store.get(k) ?? null,
    setItem: (k: string, v: string) => void store.set(k, v),
    removeItem: (k: string) => void store.delete(k),
    clear: () => store.clear()
  })
  vi.stubGlobal(
    'fetch',
    vi.fn(async () => ({ ok: true, json: async () => INDEX }))
  )
})

afterEach(() => {
  vi.unstubAllGlobals()
})

function mountPage(): { target: HTMLDivElement; comp: ReturnType<typeof mount> } {
  const target = document.createElement('div')
  document.body.appendChild(target)
  const comp = mount(Marketplace, { target })
  return { target, comp }
}

describe('Marketplace', () => {
  it('renders plugins from the registry with install state', async () => {
    const { target, comp } = mountPage()
    try {
      await vi.waitFor(() => {
        expect(target.textContent).toContain('Alpha')
        expect(target.textContent).toContain('Zen WARP')
      })
      // Default registry label (pragma/plugins) is shown.
      expect(target.textContent).toContain('pragma/plugins')
      // Not installed → Install button; installed id → flagged with version.
      expect(target.textContent).toContain('Installed v2.4.1')
      const installButtons = [...target.querySelectorAll('button')].filter((b) =>
        b.textContent?.includes('Install')
      )
      expect(installButtons.length).toBeGreaterThan(0)
    } finally {
      unmount(comp)
      target.remove()
    }
  })

  it('filters plugins by search', async () => {
    const { target, comp } = mountPage()
    try {
      await vi.waitFor(() => expect(target.textContent).toContain('Alpha'))
      const input = target.querySelector('input.search') as HTMLInputElement
      expect(input).toBeTruthy()
      input.value = 'alph'
      input.dispatchEvent(new Event('input', { bubbles: true }))
      await vi.waitFor(() => {
        expect(target.textContent).toContain('Alpha')
        expect(target.textContent).not.toContain('Zen WARP')
      })
    } finally {
      unmount(comp)
      target.remove()
    }
  })

  it('adds a custom registry and fetches its index', async () => {
    const { target, comp } = mountPage()
    try {
      await vi.waitFor(() => expect(target.textContent).toContain('Alpha'))
      // Open the add-registry row (bubbles: true — Svelte 5 delegates events
      // at the root, so synthetic non-bubbling events never reach handlers).
      ;[...target.querySelectorAll('button')]
        .find((b) => b.textContent?.includes('Add registry'))
        ?.dispatchEvent(new Event('click', { bubbles: true }))
      const input = await vi.waitFor(() => {
        const el = target.querySelector('.add-row input') as HTMLInputElement | null
        expect(el).toBeTruthy()
        return el!
      })
      const url = 'https://raw.githubusercontent.com/someone/my-plugins/main/index.json'
      input.value = url
      input.dispatchEvent(new Event('input', { bubbles: true }))
      ;[...target.querySelectorAll('button')]
        .find((b) => b.textContent?.trim() === 'Add')
        ?.dispatchEvent(new Event('click', { bubbles: true }))
      await vi.waitFor(() => {
        expect(target.textContent).toContain('someone/my-plugins')
        expect(fetch).toHaveBeenCalledWith(url)
      })
    } finally {
      unmount(comp)
      target.remove()
    }
  })

  it('survives a failing registry fetch without crashing', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => ({ ok: false, status: 404 })))
    const { target, comp } = mountPage()
    try {
      // The empty state shows immediately; the error chip appears once the
      // failing fetch settles.
      await vi.waitFor(() => expect(target.textContent).toContain('HTTP 404'))
      expect(target.textContent).toContain('No plugins loaded')
    } finally {
      unmount(comp)
      target.remove()
    }
  })
})
