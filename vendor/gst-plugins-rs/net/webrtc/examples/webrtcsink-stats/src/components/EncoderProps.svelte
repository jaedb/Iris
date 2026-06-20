<script lang="ts">
  import type { ConsumerType } from '@/types/app'
  import { MitigationMode } from '@/types/app'
  import Fa from 'svelte-fa'
  import { faExclamationTriangle, faCheckCircle } from '@fortawesome/free-solid-svg-icons';
  import vp8_logo from '@/assets/vp8.png'
  import vp9_logo from '@/assets/vp9.png'
  import h264_logo from '@/assets/h264.png'

  export let consumer: ConsumerType

  $: video_codec = consumer.video_codec
  $: mitigation_mode = consumer.mitigation_mode
</script>

<div class="encoder-props">
  <div class="codec">
    {#if video_codec == "video/x-vp8"}
      <img src={vp8_logo} alt="VP8">
    {:else if video_codec == "video/x-vp9"}
      <img src={vp9_logo} alt="VP8">
    {:else if video_codec == "video/x-h264"}
      <img src={h264_logo} alt="VP8">
    {/if}
  </div>
  <div>
    {#if mitigation_mode & MitigationMode.Downsampled && mitigation_mode & MitigationMode.Downscaled}
      <abbr title="Very congested link, video is downscaled and downsampled">
        <Fa icon={faExclamationTriangle} color="tomato" />
      </abbr>
    {:else if mitigation_mode & MitigationMode.Downscaled}
      <abbr title="Congested link, video is downscaled">
        <Fa icon={faExclamationTriangle} color="orange" />
      </abbr>
    {:else}
      <abbr title="Link with minimal to no congestion">
        <Fa icon={faCheckCircle} color="lightseagreen" />
      </abbr>
    {/if}
  </div>
</div>

<style lang="scss">
  .encoder-props {
    display: flex;
    justify-content: space-evenly;
    .codec {
      img {
        width: 25px;
      }
    }
  }
</style>
