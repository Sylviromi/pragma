<script lang="ts">
  // Fonts — Barlow / Barlow Condensed / JetBrains Mono + Material Symbols Rounded
  // (bundled locally via npm so the desktop app works fully offline).
  import 'material-symbols/rounded.css'
  import '@fontsource/barlow/400.css'
  import '@fontsource/barlow/500.css'
  import '@fontsource/barlow/700.css'
  import '@fontsource/barlow-condensed/400.css'
  import '@fontsource/barlow-condensed/600.css'
  import '@fontsource/jetbrains-mono/400.css'
  import '@fontsource/jetbrains-mono/500.css'
  import '@fontsource/jetbrains-mono/600.css'
  import '../styles/tokens.css'
  import '../styles/base.css'
  import TabBar from '../components/TabBar.svelte'
  import StatusBar from '../components/StatusBar.svelte'
  import Toast from '../components/Toast.svelte';
  import PluginConfirmModal from '../components/PluginConfirmModal.svelte';
  import { onMount } from 'svelte';
  import { loadPlugins, initPluginEvents } from '../stores/plugins.svelte';
  import { initStatsEvents } from '../stores/stats.svelte';
  import { initTheme, applyTheme, theme } from '../stores/theme.svelte';

  let { children } = $props();

  onMount(() => {
    initTheme();
    initPluginEvents();
    loadPlugins();
    initStatsEvents();
  });

  // Re-apply the family×mode token set whenever the user changes it; also
  // reacts to matchSystem/reduceMotion so <html> attributes stay in sync.
  $effect(() => {
    const _ = [theme.family, theme.mode, theme.matchSystem, theme.reduceMotion];
    applyTheme();
  });
</script>

<div id="app">
  <TabBar />
  <StatusBar />
  <main>{@render children()}</main>
  <Toast />
  <PluginConfirmModal />
</div>

<style>
  #app {
    display: flex;
    flex-direction: column;
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