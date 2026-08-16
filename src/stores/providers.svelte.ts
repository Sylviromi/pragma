import { listProviders as fetch } from '../lib/tauri';
import { listen } from '@tauri-apps/api/event';
import type { ProviderInfo } from '../lib/types';

export const providerState = $state({ providers: [] as ProviderInfo[] });

export async function loadProviders() {
  providerState.providers = await fetch();
}

let eventsInited = false;
/** Subscribe to backend model-discovery updates so newly fetched models
 *  (after an API key is saved or discovery resolves) appear live. */
export function initProviderEvents() {
  if (eventsInited) return;
  eventsInited = true;
  listen('providers:changed', async () => {
    await loadProviders();
  });
}
