<script lang="ts">
  import { untrack } from 'svelte'
  import MessageList from './MessageList.svelte'
  import InputArea from './InputArea.svelte'
  import { loadMessages } from '../stores/chat.svelte'

  let { sessionId }: { sessionId: string } = $props()

  // Load messages for the active session, and reload when the user switches
  // tabs (sidebar / tab bar). untrack() keeps the effect depending on
  // sessionId only — loadMessages reads chat state, which would otherwise
  // re-trigger it on every messages change.
  $effect(() => {
    const id = sessionId
    untrack(() => loadMessages(id))
  })
</script>

<div class="panel">
  <MessageList {sessionId} />
  <InputArea {sessionId} />
</div>

<style>
  .panel {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0; /* let children shrink instead of overflowing */
    overflow: hidden;
  }
</style>
