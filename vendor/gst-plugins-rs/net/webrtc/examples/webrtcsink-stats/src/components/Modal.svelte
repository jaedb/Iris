<script lang="ts">
  import Fa from 'svelte-fa'
  import { faTimes } from '@fortawesome/free-solid-svg-icons';
  import { createEventDispatcher } from 'svelte';

  const dispatch = createEventDispatcher();
</script>

<div class="modal-overlay">
  <div class="modal">
    <div class="modal-header">
      <slot name="title"></slot>
      <div class="close-icon" on:click="{() => dispatch('closeModal')}">
        <Fa icon={faTimes} color="#afaeae" size="1x" />
      </div>
    </div>

    <slot name="body"></slot>

    <slot name="footer"></slot>
  </div>
</div>

<style lang="scss">
  .modal {
    background-color: white;
    padding: 15px 0;
    border-radius: 10px;
    box-shadow: rgb(0 0 0 / 24%) 0px 3px 8px;
    &-overlay {
      position: absolute;
      top: 0;
      height: 100vh;
      width: 100vw;
      display: flex;
      align-items: center;
      justify-content: center;
      background-color: rgb(0, 0, 0, 0.4);
    }
    &-header {
      display: flex;
      justify-content: space-between;
      padding: 0 15px 10px;
      border-bottom: 3px solid #e2e2e2;
    }
    .close-icon {
      cursor: pointer;
    }
  }
</style>
