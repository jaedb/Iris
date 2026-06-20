<script lang="ts">
  import type { ConsumerType } from '@/types/app'
  import Consumer from '@/components/Consumer.svelte'
  import PlotConsumerModal from '@/components/PlotConsumerModal.svelte'

  export let consumers: Array<ConsumerType>

  let consumerToPlot: ConsumerType | undefined
  let showPlotModal = false

  /**
   * Display the Plot modal
   *
   * @param {ConsumerType} consumer
   */
  const openPlotConsumer = (consumer: ConsumerType) => {
    consumerToPlot = consumer
    showPlotModal = true
  }

  /**
   * Close the Plot modal
   *
   */
  const closePlotConsumer = () => {
    consumerToPlot = undefined
    showPlotModal = false
  }
</script>

<main>
  <div class="consumer-card-container">
    {#each consumers as consumer}
      <Consumer
        consumer = {consumer}
        on:click="{() => { openPlotConsumer(consumer) }}"
      />
    {/each}
  </div>
</main>

{#if showPlotModal}
  <PlotConsumerModal
    consumer={consumers.find(consumer => consumer == consumerToPlot)}
    on:close="{closePlotConsumer}"
  />
{/if}

<style lang="scss">
  main {
    padding: 2em;
    margin: 0 auto;
    width: 100vw;
    box-sizing: border-box;
  }
  .consumer-card {
    &-container {
      display: flex;
      flex-wrap: wrap;
      row-gap: 20px;
      justify-content: space-evenly;
    }
  }
</style>
