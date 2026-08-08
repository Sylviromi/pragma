<script lang="ts">
  import { onMount } from 'svelte'
  import MessageList from './MessageList.svelte'
  import InputArea from './InputArea.svelte'
  import { loadMessages } from '../stores/chat.svelte'

  let { sessionId }: { sessionId: string } = $props()

  onMount(() => loadMessages(sessionId))
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
    overflow: hidden;
  }
</style>
