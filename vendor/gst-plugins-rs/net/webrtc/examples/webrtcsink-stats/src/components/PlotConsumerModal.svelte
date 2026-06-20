<svelte:head>
  <script src="https://cdn.plot.ly/plotly-latest.min.js" type="text/javascript"></script>
</svelte:head>

<script lang="ts">
  import { createEventDispatcher, onMount, onDestroy } from 'svelte';
  import Modal from '@/components/Modal.svelte'
  import type { ConsumerType } from '@/types/app'
  import EncoderProps from '@/components/EncoderProps.svelte'

  export let consumer: ConsumerType

  $: if (consumer === undefined) {
    dispatch('close')
  }

  $: id = consumer !== undefined ? consumer.id : undefined

  const dispatch = createEventDispatcher();
  let interval: ReturnType<typeof setInterval> | undefined = undefined

  onMount(() => {
    let plotDiv = document.getElementById('plotDiv');				

    let traces = []
    let layout = {
      legend: {traceorder: 'reversed'},
      height: 800,
    }
    let ctr = 1;
    let domain_step = 1.0 / consumer.stats.size
    let domain_margin = 0.05

    for (let key of consumer.stats.keys()) {
      let trace = {
        x: [],
        y: [],
        xaxis: 'x' + ctr,
        yaxis: 'y' + ctr,
        mode: 'lines',
        line: {shape: 'spline'},
        name: key
      }

      traces.push(trace)

      layout['xaxis' + ctr] = {
        type: 'date',
      }

      layout['yaxis' + ctr] = {
        domain: [(ctr - 1) * domain_step, (ctr * domain_step) - domain_margin],
        rangemode: "tozero",
      }

      ctr += 1
    }

    Plotly.newPlot(plotDiv, traces, layout);

    interval = setInterval(function() {
      let time = new Date()
      let ctr = 0
      let traces = []
      let data_update = {
        x: [],
        y: [],
      }

      for (let value of Object.values(consumer.stats)) {
        data_update.x.push([time])
        data_update.y.push([value])
        traces.push(ctr)
        ctr += 1
      }

      Plotly.extendTraces(plotDiv, data_update, traces, 600)

    }, 50);
  });

  onDestroy(() => {
    console.log ("destroyed")

    if (interval !== undefined ) {
      clearInterval (interval)
      interval = undefined
    }
  })
</script>

<Modal on:closeModal="{() => dispatch('close')}">
  <div slot="body" class="modal-body">
    <div class="id">{id}</div>
    <EncoderProps
      consumer={consumer}
    />
    <div id="plotDiv"></div>
  </div>

  <div slot="footer" class="modal-footer">
    <div class="buttons-wrapper">
      <button
        class="button"
        on:click|stopPropagation="{() => dispatch('close')}"
      >
        Cancel
    </button>
    </div>
  </div>
</Modal>

<style lang="scss">
  .modal {
    &-body {
      width: 1000px;
      padding: 20px 15px 10px;
      gap: 15px 0;
      .id {
        font-weight: bold;
        margin-bottom: 10px;
      }
    }
    &-footer {
      padding: 0 15px;
      .buttons-wrapper {
        text-align: right;
      }
      .button {
        height: 30px;
        padding: 0 10px;
        text-align: center;
        box-sizing: content-box;
        border-radius: 3px;
        border: 1px solid #000;
        &:active {
          background-color: #b9b7b7;
        }
      }
    }
  }
</style>
