import { listProviders as fetch } from '../lib/tauri'
import type { ProviderInfo } from '../lib/types'

export const providerState = $state({ providers: [] as ProviderInfo[] })

export async function loadProviders() {
  providerState.providers = await fetch()
}