# gst-plugins-rs [![crates.io](https://img.shields.io/crates/v/gst-plugin.svg)](https://crates.io/crates/gst-plugin) [![pipeline status](https://gitlab.freedesktop.org/gstreamer/gst-plugins-rs/badges/main/pipeline.svg)](https://gitlab.freedesktop.org/gstreamer/gst-plugins-rs/commits/main)

Repository containing various [GStreamer](https://gstreamer.freedesktop.org/)
plugins and elements written in the [Rust programming
language](https://www.rust-lang.org/).

The plugins build upon the [GStreamer Rust bindings](https://gitlab.freedesktop.org/gstreamer/gstreamer-rs).
Check the README.md of that repository also for details about how to set-up
your development environment.

## Releases and git tags

Releases from this repository are done from the versioned release branches,
i.e. `0.14` right now, and there's a tag for each release, e.g. `0.14.2`.
Plugins that are ready for general usage are also published to [crates.io](https://crates.io).

Distributors should make use of these versions.

In addition there are `gstreamer-X.Y.Z` tags in this repository. These tags
are for internal use by [cerbero](https://gitlab.freedesktop.org/gstreamer/cerbero) and are
used for the binary releases published by the GStreamer project. These tags
are not supposed to be used by distributors.

## Plugins

You will find the following plugins in this repository:

  * `generic`

    - `file`: A Rust implementation of the standard `filesrc` and `filesink` elements

    - `gopbuffer`: Stores a minimum duration of data delimited by discrete GOPs (Group of Picture).

    - `inter`: 1:N wormhole for sending data from one pipeline to another within the same process using the [`StreamProducer` API](https://docs.rs/gstreamer-utils/latest/gstreamer_utils/struct.StreamProducer.html).

        - `intersink`: send data to one or more `intersrc` within the same process.

        - `intersrc`: receive data from an `intersink` in the same process.

    - `originalbuffer`:

        - `originalbuffersave`: Saves a reference to the buffer in a meta so it can later be restored again after transformations such as downscaling before inference.

        - `originalbufferrestore`: Restores the original buffer previously saved by `originalbuffersave`.

    - `sodium`: Elements to perform encryption and decryption using [libsodium](https://libsodium.org).

    - `streamgrouper`: Filter element that makes all the incoming streams use the same group-id.

    - `threadshare`: Some popular threaded elements reimplemented using common thread-sharing infrastructure.

  * `net`

    - `aws`: Various elements for Amazon AWS services using the [AWS SDK](https://awslabs.github.io/aws-sdk-rust/) library
      - `s3src`/`s3sink`: A source and sink element to talk to the Amazon S3 object storage system.
      - `s3putobjectsink`: A sink element to talk to Amazon S3. Uses `PutObject` instead of multi-part upload like `s3sink`.
      - `s3hlssink`: A sink element to store HLS streams on Amazon S3.
      - `awstranscriber`: an element wrapping the AWS Transcriber service.
      - `awstranscribeparse`: an element parsing the packets of the AWS Transcriber service.

    - `deepgram`: Wrapper elements to talk to the [Deepgram API](https://developers.deepgram.com/home)
      - `deepgramtranscriber`: an element wrapping the Deepgram Speech-to-Text service

    - `hlsmultivariantsink`: Create multi-variant HLS playlists with alternate renditions and variant streams.

    - `hlssink3`: An element for generating MPEG-TS HLS streams.

    - `mpegtslive`:

      - `mpegtslivesrc`: Wraps MPEG-TS sources such as `udpsrc` or `srtsrc` and provides a live clock based on the stream's PCR.

    - `ndi`: An [NDI](https://www.newtek.com/ndi/) plugin containing a source, sink and device provider.

    - `onvif`: Various elements for parsing, RTP (de)payloading, overlaying of ONVIF timed metadata.

    - `quinn`: Transfer data over the network using QUIC
      - `quinnquicsink`/`quinnquicsrc`: Send and receive data using QUIC
      - `quinnquicmux`/`quinnquicdemux`: Multiplex and de-multiplex streams and datagram using QUIC
      - `quinnroqmux`/`quinnroqdemux`: Multiplex and de-multiplex RTP streams over QUIC

    - `raptorq`: Encoder/decoder element for RaptorQ RTP FEC mechanism.

    - `reqwest`: An HTTP source element based on the [reqwest](https://github.com/seanmonstar/reqwest) library.

    - `rtp`:
      - `rtpav1pay` / `rtpav1depay`: RTP (de)payloader for the AV1 video codec.

      - `rtpgccbwe`: RTP bandwidth estimator based on the Google Congestion Control algorithm.

    - `rtsp`:

      - `rtspsrc2`: New Rust implementation of a Real Time Streaming Protocol (RTSP) (RFC 2326, 7826) source element.

    - `webrtc`: WebRTC elements, with batteries included Sink elements for specific signalling protocols.

    - `webrtchttp`: Simple WebRTC HTTP elements (WHIP/WHEP).

  * `audio`
    - `audiofx`: Elements to apply audio effects to a stream
      - `rsaudioecho`: a simple echo/reverb filter.
      - `audioloudnorm`: [audio normalization](http://k.ylo.ph/2016/04/04/loudnorm.html) filter.
      - `audiornnoise`: Filter for [removing noise](https://jmvalin.ca/demo/rnnoise/).
      - `ebur128level`: Filter for measuring audio loudness according to EBU R-128.
      - `hrtfrender`: Filter for rendering audio according to a [head-related transfer
        function](https://en.wikipedia.org/wiki/Head-related_transfer_function).
    - `audioparsers`: Audio parser elements
      - `s302mparse`: Parser for SMPTE S302M audio elementary streams.

    - `claxon`: A FLAC decoder based on the [Claxon](https://github.com/ruuda/claxon) library.

    - `csound`: A plugin to implement audio effects using the [Csound](https://csound.com/) library.

    - `elevenlabs`:

      - `elevenlabssynthesizer`: Generate audio speech from text using the [ElevenLabs](https://elevenlabs.io) API/service.

    - `lewton`: A Vorbis decoder based on the [lewton](https://github.com/RustAudio/lewton) library.

    - `speechmatics`:

      - `speechmaticstranscriber`: Speech to text transcription using [Speechmatics](https://www.speechmatics.com/speech-to-text)

    - `spotify`: A plugin to access content from [Spotify](https://www.spotify.com/) based on the [librespot](https://github.com/librespot-org/) library.

  * `video`
    - `cdg`: A parser and renderer for [CD+G karaoke data](https://docs.rs/cdg/0.1.0/cdg/).

    - `closedcaption`: Plugin to deal with closed caption streams
      - `ccdetect`: Detects if a stream contains active Closed Captions.
      - `cctost2038anc`: Convert closed captions to ST-2038 ANC.
      - `cdpserviceinject`: Add or update service description data in a CDP.
      - `cea608overlay`: Overlay CEA-608 / EIA-608 closed captions over a
        video stream.
      - `cea608tocea708`: Upconvert a CEA-608 / EIA-608 caption stream to the equivalant
        CEA-708 caption stream.
      - `cea608tojson`: Convert CEA-608 / EIA-608 closed captions to a JSON
        stream.
      - `cea608tott`: Convert CEA-608 / EIA-608 closed captions to timed text.
      - `cea708mux`: Mux multiple CTA-708 / CEA-708 services together.
      - `cea708overlay`: Overlay CTA-708 / CEA-708 closed captions over a video
        stream.
      - `jsontovtt`: Convert JSON to timed text.
      - `mccenc`: Convert CEA-608 / EIA-608 and CEA-708 / EIA-708 closed captions to the MCC format.
      - `mccparse`: Parse CEA-608 / EIA-608 and CEA-708 / EIA-708 closed captions from the MCC format.
      - `sccenc`: Convert CEA-608 / EIA-608 closed captions to the MCC format.
      - `sccparse`: Parse CEA-608 / EIA-608 closed captions from the MCC format.
      - `st2038ancdemux`: Split individual ancillary streams from a ST-2038
        stream.
      - `st2038ancmux`: Mux togehter multiple ancillary ST-2038 streams.
      - `st2038tocc`: Convert an ancillary ST-2038 stream into closed captions.
      - `transcriberbin`: Convenience bin around transcriber elements like `aws_transcriber`.
      - `translationbin`: Convenience bin around transcriber and translation
        elements.
      - `tttocea608`: Convert timed text to CEA-608 / EIA-608 closed captions.
      - `tttocea708`: Convert timed text to CTA-708 / CEA-708 closed captions.
      - `tttojson`: Convert timed text to JSON.

    - `dav1d`: AV1 decoder based on the [dav1d](https://code.videolan.org/videolan/dav1d) library.

    - `ffv1`: FFV1 decoder based on the [ffv1](https://github.com/rust-av/ffv1) library.

    - `gif`: A GIF encoder based on the [gif](https://github.com/image-rs/image-gif) library.

    - `gtk4`: A [GTK4](https://www.gtk.org) video sink that provides a `GdkPaintable` for UI integration.

    - `hsv`: Plugin with various elements to work with video data in hue, saturation, value format
       - `hsvdetector`: Mark pixels that are close to a configured color in HSV format.
       - `hsvfilter`: Apply various transformations in the HSV colorspace.

    - `png`: PNG encoder based on the [png](https://github.com/image-rs/image-png) library.

    - `rav1e`: AV1 encoder based on the [rav1e](https://github.com/xiph/rav1e) library.

    - `skia`:

      - `skiacompositor`: Video compositor based on [Skia](https://skia.org) graphics library.

    - `videofx`: Plugin with various video filters.
      - `roundedcorners`: Element to make the corners of a video rounded via the alpha channel.
      - `colordetect`: A pass-through filter able to detect the dominant color(s) on incoming frames, using [color-thief](https://github.com/RazrFalcon/color-thief-rs).
      - `videocompare`: Compare similarity of video frames. The element can use different hashing algorithms like [Blockhash](https://github.com/commonsmachinery/blockhash-rfc), [DSSIM](https://kornel.ski/dssim), and others.

    - `vvdec`: VVC/H.266 decoder based on [VVdeC](https://github.com/fraunhoferhhi/vvdec), the Fraunhofer Versatile Video Decoder.

    - `webp`: WebP decoder based on the [libwebp-sys-2](https://github.com/qnighy/libwebp-sys2-rs) library.

  * `mux`
    - `flavors`: FLV demuxer based on the [flavors](https://github.com/rust-av/flavors) library.

    - `isobmff`: A MP4/ISOBMFF/CMAF muxer for generating fragmented (e.g. DASH/HLS media) and non-fragmented (MP4) files.

  * `text`
    - `accumulate`: A plugin for segmenting text, designed to work in a live context

    - `ahead`: A plugin to display upcoming text buffers ahead.

    - `json`: A plugin to convert a stream of JSON objects to a higher level wrapped NDJSON output.

    - `regex`: A regular expression text filter plugin.

    - `wrap`: A plugin to perform text wrapping with hyphenation.

  * `utils`
    - `fallbackswitch`:
      - `fallbackswitch`: An element that allows falling back to different
        sink pads after a timeout based on the sink pads' priorities.
      - `fallbacksrc`: Element similar to `urisourcebin` that allows
        configuring a fallback audio/video if there are problems with the main
        source.

    - `livesync`: Element to maintain a continuous live stream from a
      potentially unstable source.

    - `togglerecord`: Element to enable starting and stopping multiple streams together.

    - `tracers`: Plugin with multiple tracers:
      - `buffer-lateness`: Records lateness of buffers and the reported
        latency for each pad in a CSV file. Contains a script for
        visualization.
      - `pipeline-snapshot`: Creates a .dot file of all pipelines in the
        application whenever requested.
      - `queue-levels`: Records queue levels for each queue in a CSV file.
        Contains a script for visualization.

    - `uriplaylistbin`: Helper bin to gaplessly play a list of URIs.

  * `analytics`

    - `analytics`:

      - `analyticscombiner`: Analytics combiner / batcher element

      - `analyticssplitter`: Analytics batch splitter element

      - `onvifmeta2relationmeta`: Convert ONVIF metadata to relation metas

      - `relationmeta2onvifmeta`: Convert relation metadata to ONVIF metas

## Building

gst-plugins-rs relies on [cargo-c](https://github.com/lu-zero/cargo-c/) to
generate shared and static C libraries. It can be installed using:

```
$ cargo install cargo-c
```

Then you can easily build and test a specific plugin:

```
$ cargo cbuild -p gst-plugin-cdg
$ GST_PLUGIN_PATH="target/x86_64-unknown-linux-gnu/debug:$GST_PLUGIN_PATH" gst-inspect-1.0 cdgdec
```

Replace `x86_64-unknown-linux-gnu` with your system's Rust target triple (`rustc -vV`).

The plugin can also be installed system-wide:

```
$ cargo cinstall -p gst-plugin-cdg --prefix=/usr
```

This will install the plugin to `/usr/lib/gstreamer-1.0`.
You can use `--libdir` to pass a custom `lib` directory
such as `/usr/lib/x86_64-linux-gnu` for example.

Note that you can also just use `cargo` directly to build Rust static libraries
and shared C libraries. `cargo-c` is mostly useful to build static C libraries
and generate `pkg-config` files.

In case cargo complains about dependency versions after a `git pull`, `cargo update` may
be able to resolve those.

## LICENSE

gst-plugins-rs and all crates contained in here are licensed under one of the
following licenses

 * Mozilla Public License Version 2.0 ([LICENSE-MPL-2.0](LICENSE-MPL-2.0) or http://opensource.org/licenses/MPL-2.0)
 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

GStreamer itself is licensed under the Lesser General Public License version
2.1 or (at your option) any later version: https://www.gnu.org/licenses/lgpl-2.1.html

## Contribution

Any kinds of contributions are welcome as a merge request.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in gst-plugins-rs by you shall be licensed under the license of
the plugin it is added to.

For new plugins the MPL-2 license is preferred.
