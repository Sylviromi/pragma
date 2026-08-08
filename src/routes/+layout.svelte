<script lang="ts">
  import '../styles/dark.css';
  import '../styles/light.css';
  import '../styles/high-contrast.css';
  import '../styles/base.css';
  import TabBar from '../components/TabBar.svelte';
  import { onMount } from 'svelte';

  let { children } = $props();

  let theme = $state<'dark' | 'light' | 'high-contrast'>('dark');

  // Restore saved theme on startup (plan: restore theme before chat loads)
  onMount(() => {
    const saved = localStorage.getItem('pragma:theme');
    if (saved) theme = saved as 'dark' | 'light' | 'high-contrast';
  });

  $effect(() => document.documentElement.setAttribute('data-theme', theme));
</script>

<div id="app">
  <TabBar />
  <main>{@render children()}</main>
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
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }
</style>