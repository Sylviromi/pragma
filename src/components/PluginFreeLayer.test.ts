import { describe, it, expect, beforeEach } from 'vitest';
import { mount, unmount, tick } from 'svelte';
import PluginFreeLayer from './PluginFreeLayer.svelte';
import { plugins } from '../stores/plugins.svelte';
import type { PluginInfo } from '../lib/types';

const mk = (over: Partial<PluginInfo>): PluginInfo => ({
  id: 'p',
  name: 'P',
  version: '1',
  description: '',
  author: '',
  publisher: '',
  repository: '',
  homepage: '',
  updateUrl: '',
  enabled: true,
  contributes: { providers: [], slots: [], commands: [], skills: [] },
  hasComponent: true,
  hasSkill: false,
  skills: [],
  folder: '/tmp/p',
  source: null,
  ...over,
});

const slots = (slots: string[]) => ({ providers: [], slots, commands: [], skills: [] });

beforeEach(() => {
  plugins.list = [];
});

describe('PluginFreeLayer', () => {
  it('mounts enabled plugins that claim unanchored slots', async () => {
    plugins.list = [mk({ id: 'frontend-dev', contributes: slots(['miniscreen']) })];
    const target = document.createElement('div');
    document.body.appendChild(target);
    const comp = mount(PluginFreeLayer, { target });
    await tick();

    expect(target.querySelector('.plugin-free-layer')).not.toBeNull();
    expect(target.querySelector('[data-plugin="frontend-dev"]')).not.toBeNull();
    expect(target.querySelector('[data-slot="miniscreen"]')).not.toBeNull();

    unmount(comp);
    target.remove();
  });

  it('ignores plugins whose slots are all anchored', async () => {
    plugins.list = [
      mk({ id: 'pragma-dev', contributes: slots(['status-bar']) }),
      mk({ id: 'convo', contributes: slots(['chat-view', 'settings-page']) }),
    ];
    const target = document.createElement('div');
    document.body.appendChild(target);
    const comp = mount(PluginFreeLayer, { target });
    await tick();

    expect(target.querySelector('.plugin-free-layer')).toBeNull();

    unmount(comp);
    target.remove();
  });

  it('skips disabled plugins and plugins without a component', async () => {
    plugins.list = [
      mk({ id: 'off', enabled: false, contributes: slots(['pets']) }),
      mk({ id: 'nocomp', hasComponent: false, contributes: slots(['pets']) }),
    ];
    const target = document.createElement('div');
    document.body.appendChild(target);
    const comp = mount(PluginFreeLayer, { target });
    await tick();

    expect(target.querySelector('.plugin-free-layer')).toBeNull();

    unmount(comp);
    target.remove();
  });

  it('reacts when plugins load after mount', async () => {
    const target = document.createElement('div');
    document.body.appendChild(target);
    const comp = mount(PluginFreeLayer, { target });
    await tick();
    expect(target.querySelector('.plugin-free-layer')).toBeNull();

    // Simulate loadPlugins() resolving after the layout mounted.
    plugins.list = [mk({ id: 'frontend-dev', contributes: slots(['miniscreen']) })];
    await tick();
    expect(target.querySelector('.plugin-free-layer')).not.toBeNull();
    expect(target.querySelector('[data-plugin="frontend-dev"]')).not.toBeNull();

    unmount(comp);
    target.remove();
  });
});
