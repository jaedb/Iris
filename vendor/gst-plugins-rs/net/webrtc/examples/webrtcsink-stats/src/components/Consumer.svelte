<script lang="ts">
  import type { ConsumerType } from '@/types/app'
  import EncoderProps from '@/components/EncoderProps.svelte'

  export let consumer: ConsumerType

  $: id = consumer.id
</script>

<div class="consumer-card" on:click >
  <div class="id">{id}</div>
  <EncoderProps
    consumer={consumer}
  />
</div>

<style lang="scss">
  .consumer-card {
    word-break: break-all;
    width: 150px;
    height: 100px;
    margin-right: 15px;
    background-color: #fff;
    padding: 15px;
    border-radius: 10px;
    box-shadow: rgb(0 0 0 / 24%) 0px 3px 8px;
    .id {
      font-weight: bold;
      margin-bottom: 10px;
    }
  }
</style>
