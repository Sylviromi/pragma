<script lang="ts">
  import { untrack } from 'svelte';
  import MessageList from './MessageList.svelte';
  import PluginSlot from './PluginSlot.svelte';
  import InputArea from './InputArea.svelte';
  import { loadMessages } from '../stores/chat.svelte';
  import { pluginsForSlot } from '../stores/plugins.svelte';

  let { sessionId }: { sessionId: string } = $props();

  const chatViewPlugin = $derived(pluginsForSlot('chat-view')[0] ?? null);

  // Load messages for the active session, and reload when the user switches
  // tabs (sidebar / tab bar). untrack() keeps the effect depending on
  // sessionId only — loadMessages reads chat state, which would otherwise
  // re-trigger it on every messages change.
  $effect(() => {
    const id = sessionId;
    untrack(() => loadMessages(id));
  });
</script>

<div class="panel">
  {#if chatViewPlugin}
    <!-- A `chat-view` plugin owns the whole panel: conversation AND composer
         (it forces its own send path, e.g. auto-approve always on). The default
         InputArea is hidden so the plugin's composer is the only one shown. -->
    <div class="chat-view-slot">
      <PluginSlot slot="chat-view" context={sessionId} pluginId={chatViewPlugin.id} />
    </div>
  {:else}
    <MessageList {sessionId} />
    <InputArea {sessionId} />
  {/if}
</div>

<style>
  .panel {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0; /* let children shrink instead of overflowing */
    overflow: hidden;
  }
  .chat-view-slot {
    flex: 1;
    min-height: 0;
    overflow: hidden;
    /* Let the plugin host flex-fill instead of relying on height:100%
       through the display:contents wrapper (collapses in WebKitGTK). */
    display: flex;
    flex-direction: column;
  }
</style>
