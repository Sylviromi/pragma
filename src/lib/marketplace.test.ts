import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { mount, unmount } from 'svelte'
import Marketplace from '../routes/settings/marketplace/+page.svelte'
import { plugins } from '../stores/plugins.svelte'
import type { PluginInfo } from './types'

// A fake npm search response — one installable package, one installed one.
const SEARCH = {
  objects: [
    {
      package: {
        name: 'pragma-plugin-alpha',
        version: '1.0.0',
        description: 'A test plugin',
        author: { name: 'Ada' },
        keywords: ['pragma-plugin']
      }
    },
    {
      package: {
        name: 'pragma-plugin-zen-warp',
        version: '2.4.1',
        description: 'already installed',
        author: 'Bob'
      }
    }
  ]
}

const installedPlugin: PluginInfo = {
  id: 'zen-warp',
  name: 'Zen WARP',
  version: '2.4.1',
  description: '',
  author: '',
  publisher: '',
  repository: '',
  homepage: '',
  updateUrl: '',
  enabled: true,
  contributes: { providers: [], slots: [], commands: [], skills: [] },
  hasComponent: false,
  hasSkill: false,
  skills: [],
  folder: '/tmp/zen-warp',
  source: 'npm:pragma-plugin-zen-warp'
}

beforeEach(() => {
  plugins.list = []
  vi.stubGlobal('fetch', vi.fn(async () => ({ ok: true, json: async () => SEARCH })))
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

describe('Marketplace (npm search)', () => {
  it('renders npm search results with install state', async () => {
    const { target, comp } = mountPage()
    try {
      await vi.waitFor(() => expect(target.textContent).toContain('pragma-plugin-alpha'))
      // onMount's loadPlugins populated the list with mock data (no source) —
      // installing state is derived from plugins.list, so set it reactively.
      plugins.list = [installedPlugin]
      await vi.waitFor(() => expect(target.textContent).toContain('Installed v2.4.1'))
      expect(target.textContent).toContain('by Ada')
      const installButtons = [...target.querySelectorAll('button')].filter((b) =>
        b.textContent?.includes('Install')
      )
      expect(installButtons.length).toBeGreaterThan(0)
    } finally {
      unmount(comp)
      target.remove()
    }
  })

  it('searches npm with the query text', async () => {
    const { target, comp } = mountPage()
    try {
      await vi.waitFor(() => expect(target.textContent).toContain('pragma-plugin-alpha'))
      const input = target.querySelector('input.search') as HTMLInputElement
      input.value = 'some-plugin'
      input.dispatchEvent(new Event('input', { bubbles: true }))
      ;[...target.querySelectorAll('button')]
        .find((b) => b.textContent?.trim() === 'Search')
        ?.dispatchEvent(new Event('click', { bubbles: true }))
      await vi.waitFor(() =>
        expect(fetch).toHaveBeenCalledWith(
          expect.stringContaining('text=some-plugin')
        )
      )
    } finally {
      unmount(comp)
      target.remove()
    }
  })

  it('survives a failing search without crashing', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => ({ ok: false, status: 500 })))
    const { target, comp } = mountPage()
    try {
      await vi.waitFor(() =>
        expect(target.textContent).toContain('No plugins found')
      )
    } finally {
      unmount(comp)
      target.remove()
    }
  })
})
