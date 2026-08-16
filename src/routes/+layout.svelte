<script lang="ts">
  // Fonts — Barlow / Barlow Condensed / JetBrains Mono + Material Symbols Rounded
  // (bundled locally via npm so the desktop app works fully offline).
  import 'material-symbols/rounded.css';
  import '@fontsource/barlow/400.css';
  import '@fontsource/barlow/500.css';
  import '@fontsource/barlow/700.css';
  import '@fontsource/barlow-condensed/400.css';
  import '@fontsource/barlow-condensed/600.css';
  import '@fontsource/jetbrains-mono/400.css';
  import '@fontsource/jetbrains-mono/500.css';
  import '@fontsource/jetbrains-mono/600.css';
  import '../styles/tokens.css';
  import '../styles/base.css';
  import { page } from '$app/stores';
  import TabBar from '../components/TabBar.svelte';
  import StatusBar from '../components/StatusBar.svelte';
  import PluginFreeLayer from '../components/PluginFreeLayer.svelte';
  import Toast from '../components/Toast.svelte';
  import PluginConfirmModal from '../components/PluginConfirmModal.svelte';
  import UpdateDialog from '../components/UpdateDialog.svelte';
  import { onMount } from 'svelte';
  import { loadPlugins, initPluginEvents, autoCheckUpdates } from '../stores/plugins.svelte';
  import { reconnectRunningSessions } from '../stores/chat.svelte';
  import { initStatsEvents } from '../stores/stats.svelte';
  import {
    initTheme,
    applyTheme,
    applyUiScale,
    applyFontFamily,
    theme,
  } from '../stores/theme.svelte';
  import { installMarkdownPluginBridge } from '../lib/markdown';
  import { initUpdater } from '../stores/updater.svelte';

  let { children } = $props();

  // The design v2 settings view owns its own chrome (logo + Settings + session
  // chips + back) — hide the app tab bar / status strip on /settings routes.
  const isSettings = $derived($page.url.pathname.startsWith('/settings'));

  onMount(() => {
    installMarkdownPluginBridge();
    initTheme();
    initPluginEvents();
    loadPlugins();
    // Fire-and-forget plugin update check at startup — populates the badge
    // in Settings → Plugins (and the nav indicator) without blocking launch.
    autoCheckUpdates();
    reconnectRunningSessions();
    initStatsEvents();
    // Auto-update: startup check (may show the one-time changelog dialog) +
    // silent 10-minute poller that toasts and lights up Settings → Update.
    initUpdater();
  });

  // Re-apply the family×mode token set whenever the user changes it; also
  // reacts to matchSystem/reduceMotion/uiScale/fontFamily so <html> attributes stay in sync.
  $effect(() => {
    const _ = [
      theme.family,
      theme.mode,
      theme.matchSystem,
      theme.reduceMotion,
      theme.uiScale,
      theme.fontFamily,
    ];
    applyTheme();
    applyUiScale();
    applyFontFamily();
  });
</script>

<div id="app">
  {#if !isSettings}
    <TabBar />
    <StatusBar />
  {/if}
  <UpdateDialog />
  <main>{@render children()}</main>
  <!-- Free-floating plugin surfaces: any plugin slot the layout doesn't
       anchor is mounted here and positions itself (position: fixed). -->
  <PluginFreeLayer />
  <Toast />
  <PluginConfirmModal />
</div>

<style>
  #app {
    display: flex;
    flex-direction: column;
    /* Root layout-zoom (base.css) already scales the viewport units, so the
       app fills the window at any interface scale. */
    height: 100vh;
    overflow: hidden;
  }
  main {
    flex: 1;
    min-height: 0;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }
</style>
