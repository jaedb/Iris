# webrtcsink examples

Collection of webrtcsink examples

## webrtcsink-stats-server

A simple application that instantiates a webrtcsink and serves stats
over websockets.

The application expects a signalling server to be running at `ws://localhost:8443`,
similar to the usage example in the main README.

``` shell
cargo run --example webrtcsink-stats-server
```

Once it is running, follow the instruction in the webrtcsink-stats folder to
run an example client.

## Rust webrtcsink-custom-signaller

An example of a custom signaller implementation in Rust, see the corresponding
[README](webrtcsink-custom-signaller/README.md) for more details on code and
usage.

## Python webrtcsink-custom-signaller

An example of a webrtcsink producer and custom signaller implemented in Python,
see [webrtcsink-custom-signaller.py](webrtcsink-custom-signaller.py). Running
the example requires the Python GStreamer bindings and the websockets library.
To install these in Debian/Ubuntu:

``` shell
sudo apt install python3-gst-1.0 python3-websockets
```

Running the Python example is similar to the process described in "[README:
Usage (standalone services)](../README#usage-standalone-services)", except
in the third step `gst-launch-1.0` is replaced with the Python program. Open
three terminals. In the first one, run the signalling server:

``` shell
cd net/webrtc/signalling
WEBRTCSINK_SIGNALLING_SERVER_LOG=debug cargo run --bin gst-webrtc-signalling-server
```

In the second one, run a web browser client:

``` shell
cd net/webrtc/gstwebrtc-api
npm install
npm start
```

In the third one, run the Python code:

``` shell
# The Python code contains a hardcoded GStreamer pipeline, so unlike
# gst-launch-1.0, there is no need to pass any extra arguments
export GST_PLUGIN_PATH=<path-to-gst-plugins-rs>/target:${GST_PLUGIN_PATH}
python3 net/webrtc/examples/webrtcsink-custom-signaller.py
```

## WebRTC precise synchronization example

This example demonstrates a sender / receiver setup which ensures precise
synchronization of multiple streams in a single session.

[RFC 6051]-style rapid synchronization of RTP streams is available as an option.
Se the [Instantaneous RTP synchronization...] blog post for details about this
mode and an example based on RTSP instead of WebRTC.

The examples can also be used for [RFC 7273] NTP or PTP clock signalling and
synchronization.

Finally, raw payloads (e.g. L24 audio) can be negotiated.

Note: you can have your host act as an NTP server, which can help the examples
with clock synchronization. For `chrony`, this can be configure by editing
`/etc/chrony.conf` and uncommenting / editing the `allow` entry. The examples
can then be launched with `--ntp-server _ip_address_`.

[RFC 6051]: https://datatracker.ietf.org/doc/html/rfc6051
[RFC 7273]: https://datatracker.ietf.org/doc/html/rfc7273
[Instantaneous RTP synchronization...]: https://coaxion.net/blog/2022/05/instantaneous-rtp-synchronization-retrieval-of-absolute-sender-clock-times-with-gstreamer/

### Signaller

The example uses the default WebRTC signaller. Launch it using the following
command:

```shell
cargo run --bin gst-webrtc-signalling-server --no-default-features
```

### Receiver

The receiver awaits for new audio & video stream publishers and render the
streams using auto sink elements. Launch it using the following command:

```shell
cargo r --example webrtc-precise-sync-recv --no-default-features
```

The default configuration should work for a local test. For a multi-host setup,
see the available options:

```shell
cargo r --example webrtc-precise-sync-recv --no-default-features -- --help
```

E.g.: the following will force `avdec_h264` over hardware decoders, activate
debug logs for the receiver and connect to the signalling server at the
specified address:

```shell
GST_PLUGIN_FEATURE_RANK=avdec_h264:MAX \
WEBRTC_PRECISE_SYNC_RECV_LOG=debug \
cargo r --example webrtc-precise-sync-recv --no-default-features -- \
  --server 192.168.1.22
```

### Sender

The sender publishes audio & video test streams. Launch it using the following
command:

```shell
cargo r --example webrtc-precise-sync-send --no-default-features
```

The default configuration should work for a local test. For a multi-host setup,
to set the number of audio / video streams, to enable rapid synchronization or
to force the video encoder, see the available options:

```shell
cargo r --example webrtc-precise-sync-send --no-default-features -- --help
```

E.g.: the following will force H264 and `x264enc` over hardware encoders,
activate debug logs for the sender and connect to the signalling server at the
specified address:

```shell
GST_PLUGIN_FEATURE_RANK=264enc:MAX \
WEBRTC_PRECISE_SYNC_SEND_LOG=debug \
cargo r --example webrtc-precise-sync-send --no-default-features -- \
  --server 192.168.1.22 --video-caps video/x-h264
```

### The pipeline latency

The `--pipeline-latency` argument configures a static latency of 1s by default.
This needs to be higher than the sum of the sender latency and the receiver
latency of the receiver with the highest latency. As this can't be known
automatically and depends on many factors, this has to be known for the overall
system and configured accordingly.

The default configuration is on the safe side and favors synchronization over
low latency. Depending on the use case, shorter or larger values should be used.

### RFC 7273 NTP or PTP clock signalling and synchronization

For [RFC 7273] NTP or PTP clock signalling and synchronization, you can use
commands such as:

#### Receiver

```shell
cargo r --example webrtc-precise-sync-recv --no-default-features -- \
  --expect-clock-signalling
```

#### Sender

```shell
cargo r --example webrtc-precise-sync-send --no-default-features -- \
  --clock ntp --do-clock-signalling \
  --video-streams 0 --audio-streams 2
```

### Raw payload

The sender can be instructed to send raw payloads. Note that raw payloads
are not activated by default and must be selected explicitly.

This command will stream two stereo L24 streams:

```shell
cargo r --example webrtc-precise-sync-send --no-default-features -- \
  --video-streams 0 \
  --audio-streams 2 --audio-codecs L24
```

Launch the receiver with:

```shell
cargo r --example webrtc-precise-sync-recv --no-default-features -- \
  --audio-codecs L24
```

This can be used to stream multiple RAW video streams using specific CAPS for
the streams and allowing fallback to VP8 & OPUS if remote doesn't support raw
payloads:

```shell
cargo r --example webrtc-precise-sync-send --no-default-features -- \
  --video-streams 2 --audio-streams 1 \
  --video-codecs RAW --video-codecs VP8 --video-caps video/x-raw,format=I420,width=400 \
  --audio-codecs L24 --audio-codecs OPUS --audio-caps audio/x-raw,rate=48000,channels=2
```

## Android

### `webrtcsrc` based Android application

An Android demonstration application which retrieves available producers from
the signaller and renders audio and video streams.

**Important**: in order to ease testing, this demonstration application enables
unencrypted network communication. See `app/src/main/AndroidManifest.xml` for
details.

#### Build the application

* Download the latest Android prebuilt binaries from:
  https://gstreamer.freedesktop.org/download/
* Uncompress / untar the package, e.g. under `/opt/android/`.
* Define the `GSTREAMER_ROOT_ANDROID` environment variable with the
  directory chosen at previous step.
* Install a recent version of Android Studio (tested with 2023.3.1.18).
* Open the project from the folder `android/webrtcsrc`.
* Have Android Studio download and install the required SDK & NDK.
* Click the build button or build and run on the target device.
* The resulting `apk` is generated under:
  `android/webrtcsrc/app/build/outputs/apk/debug`.

For more details, refer to:
* https://gstreamer.freedesktop.org/documentation/installing/for-android-development.html

Once the SDK & NDK are installed, you can use `gradlew` to build and install
the apk (make sure the device is visible from adb):

```shell
# From the android/webrtcsrc directory
./gradlew installDebug
```

#### Install the application

Prerequisites: activate developer mode on the target device.

There are several ways to install the application:

* The easiest is to click the run button in Android Studio.
* You can also install the `apk` using `adb`.

Depending on your host OS, you might need to define `udev` rules. See:
https://github.com/M0Rf30/android-udev-rules

#### Setup

1. Run the Signaller from the `gst-plugins-rs` root directory:
   ```shell
   cargo run --bin gst-webrtc-signalling-server
   ```
2. In the Android app, tap the 3 dots button -> Settings and edit the Signaller
   URI.
3. Add a producer, e.g. using `gst-launch` & `webrtcsink` or run:
   ```shell
   cargo r --example webrtc-precise-sync-send
   ```
4. Click the `Refresh` button on the Producer List view of the app.

# webrtchttp examples

Collection of webrtchttp examples

## whipserversrc and whipclientsink

An couple of examples to demonstrate how a simple pipeline with both audio and video
use these elements

1.- Run the WHIP server
```shell
  cargo r --example whipserver --features whip http://127.0.0.1:8190
```

2.- Run the WHIP client
```shell
  cargo r --example whipclient --features whip http://127.0.0.1:8190/whip/endpoint
```
