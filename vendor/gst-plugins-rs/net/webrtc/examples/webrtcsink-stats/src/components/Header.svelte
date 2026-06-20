<script lang="ts">
  import { WebSocketStatus } from '@/types/app'

  import logo from '@/assets/svelte.png'
  import Fa from 'svelte-fa'
  import { faSpinner, faExclamationTriangle, faCheckCircle } from '@fortawesome/free-solid-svg-icons';

  export let websocketStatus: WebSocketStatus
</script>

<header class="global-header">
  <div class="app-name">
    <img src={logo} alt="Svelte Logo" class="logo"/>
    <div class="title">WebRTC Stats App</div>
    <div>
      {#if websocketStatus == WebSocketStatus.Connected}
        <Fa icon={faCheckCircle} color="lightseagreen" size="1x" />
      {:else if websocketStatus == WebSocketStatus.Connecting}
        <Fa icon={faSpinner} color="#afaeae" size="1x" spin />
      {:else if websocketStatus == WebSocketStatus.Error}
        <Fa icon={faExclamationTriangle} color="tomato" size="1x" />
      {/if}
    </div>
  </div>
</header>

<style lang="scss">
  .global-header {
    background-color: #313131;
    height: 56px;
    color: white;
    padding: 15px;
    box-sizing: border-box;
    .app-name {
      display: flex;
      align-items: center;
      height: 100%;
      .logo {
        height: 100%;
        margin-right: 5px;
      }
      .title {
        font-weight: bold;
        margin-right: 5px;
      }
    }
  }
</style>
