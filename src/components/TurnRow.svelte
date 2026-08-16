<script lang="ts">
  // Assistant turn row — agent mark avatar + body column (audit A3).
  // Previously duplicated between ChatMessage (persisted turns) and
  // MessageList (live turns).
  import AgentMark from './AgentMark.svelte'
  import { sessionTint } from '../lib/tint'
  import type { Snippet } from 'svelte'

  let {
    sessionId,
    children,
  }: {
    sessionId: string
    children?: Snippet
  } = $props()
</script>

<div class="turn-row">
  <div class="avatar" aria-hidden="true"><AgentMark tint={sessionTint(sessionId)} /></div>
  <div class="turn-body">
    {@render children?.()}
  </div>
</div>

<style>
  .turn-row {
    display: flex;
    gap: 10px;
    align-items: flex-start;
  }
  .avatar {
    width: 30px;
    height: 30px;
    flex: none;
    display: flex;
    align-items: center;
    justify-content: center;
    margin-top: 2px;
  }
  .turn-body {
    flex: 1;
    min-width: 0;
    max-width: 78%;
    display: flex;
    flex-direction: column;
    gap: 7px;
  }
</style>
