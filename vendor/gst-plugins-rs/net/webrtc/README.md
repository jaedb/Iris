# webrtcsink and webrtcsrc

All-batteries included GStreamer WebRTC producer and consumer, that try their
best to do The Right Thing™.

It also provides a flexible and all-purposes WebRTC signalling server
([gst-webrtc-signalling-server](signalling/src/bin/server.rs)) and a Javascript
API ([gstwebrtc-api](gstwebrtc-api)) to produce and consume compatible WebRTC
streams from a web browser.

## Use case

The [webrtcbin] element in GStreamer is extremely flexible and powerful, but
using it can be a difficult exercise. When all you want to do is serve a fixed
set of streams to any number of consumers, `webrtcsink` (which wraps
`webrtcbin` internally) can be a useful alternative.

[webrtcbin]: https://gstreamer.freedesktop.org/documentation/webrtc/index.html

## Features

`webrtcsink` implements the following features:

* Built-in signaller: when using the default signalling server, this element
  will perform signalling without requiring application interaction.
  This makes it usable directly from `gst-launch`.

* Application-provided signalling: `webrtcsink` can be instantiated by an
  application with a custom signaller. That signaller must be a GObject, and
  must implement the `Signallable` interface as defined
  [here](src/signaller/mod.rs). The [default signaller](src/signaller/imp.rs)
  can be used as an example.

  An [example](examples/webrtcsink-custom-signaller/README.md) is also available to use as a boilerplate for
  implementing and using a custom signaller.

* Sandboxed consumers: when a consumer is added, its encoder / payloader /
  webrtcbin elements run in a separately managed pipeline. This provides a
  certain level of sandboxing, as opposed to having those elements running
  inside the element itself.

  It is important to note that at this moment, encoding is not shared between
  consumers. While this is not on the roadmap at the moment, nothing in the
  design prevents implementing this optimization.

* Congestion control: the element leverages transport-wide congestion control
  feedback messages in order to adapt the bitrate of individual consumers' video
  encoders to the available bandwidth.

* Configuration: the level of user control over the element is slowly expanding,
  consult `gst-inspect-1.0` for more information on the available properties and
  signals.

* Packet loss mitigation: webrtcsink now supports sending protection packets for
  Forward Error Correction, modulating the amount as a function of the available
  bandwidth, and can honor retransmission requests. Both features can be
  disabled via properties.

It is important to note that full control over the individual elements used by
`webrtcsink` is *not* on the roadmap, as it will act as a black box in that
respect, for example `webrtcsink` wants to reserve control over the bitrate for
congestion control.

A signal is now available however for the application to provide the initial
configuration for the encoders `webrtcsink` instantiates.

If more granular control is required, applications should use `webrtcbin`
directly, `webrtcsink` will focus on trying to just do the right thing, although
it might expose more interfaces to guide and tune the heuristics it employs.

[example project]: https://gitlab.freedesktop.org/gstreamer/gst-plugins-rs/-/tree/main/net/webrtc/examples/webrtcsink-custom-signaller

## Building

> Make sure to install the development packages for some codec libraries
> beforehand, such as libx264, libvpx and libopusenc, exact names depend
> on your distribution.

> In order to build `awskvswebrtcsink` you will need to enable the `aws` feature,
> to build `livekitwebrtcsink` and `livekitwebrtcsrc` you will need to enable the
> `livekit` feature, using the `--features` argument of `cargo build`.

``` shell
cargo build
npm --prefix gstwebrtc-api/ run build
```

## Usage (embedded services)

`webrtcsink` can optionally instantiate a signalling server and a web server.

This is the simplest set up for testing, but may not always be desirable.
For instance one may prefer hosting the services on different machines, or would
prefer that a crash from the host webrtcsink doesn't take down signalling / websites.

Head over to the following section if you want to learn how to run services individually.

To use the embedded web server, from the root of `net/webrtc` crate, first execute,

``` shell
cd gstwebrtc-api
npm install
npm run build
```

This generates the necessary artifacts in the `gstwebrtc-api/dist` directory.

In the terminal, from the root of the `net/webrtc` crate:

```
gst-launch-1.0 videotestsrc ! webrtcsink run-signalling-server=true run-web-server=true
```

In your browser of choice, navigate to <http://127.0.0.1:8080/>, and click on the stream
identifier under "Remote streams". You should see a test video stream and hear a test tone.

## Usage (standalone services)

Open three terminals. In the first one, run the signalling server:

``` shell
cd signalling
WEBRTCSINK_SIGNALLING_SERVER_LOG=debug cargo run --bin gst-webrtc-signalling-server
```

In the second one, run a web browser client (can produce and consume streams):

``` shell
cd gstwebrtc-api
npm install
npm start
```

In the third one, run a webrtcsink producer from a GStreamer pipeline:

``` shell
export GST_PLUGIN_PATH=<path-to-gst-plugins-rs>/target/debug:$GST_PLUGIN_PATH
gst-launch-1.0 webrtcsink name=ws meta="meta,name=gst-stream" videotestsrc ! ws. audiotestsrc ! ws.
```

The webrtcsink produced stream will appear in the former web page
(automatically opened at https://localhost:9090) under the name "gst-stream",
if you click on it you should see a test video stream and hear a test tone.

You can also produce WebRTC streams from the web browser and consume them with
a GStreamer pipeline. Click on the "Start Capture" button and copy the
"Client ID" value.

Then open a new terminal and run:

``` shell
export GST_PLUGIN_PATH=<path-to-gst-plugins-rs>/target/debug:$GST_PLUGIN_PATH
gst-launch-1.0 playbin3 uri=gstwebrtc://127.0.0.1:8443?peer-id=[Client ID]
```

Replacing the "peer-id" value with the previously copied "Client ID" value. You
should see the playbin3 element opening a window and showing you the content
produced by the web page.

## Configuration

The webrtcsink element itself can be configured through its properties, see
`gst-inspect-1.0 webrtcsink` for more information about that, in addition the
default signaller also exposes properties for configuring it, in
particular setting the signalling server address, those properties
can be accessed through the `gst::ChildProxy` interface, for example
with gst-launch:

``` shell
gst-launch-1.0 webrtcsink signaller::uri="ws://127.0.0.1:8443" ..
```

### Enable 'navigation' a.k.a user interactivity with the content

`webrtcsink` implements the [`GstNavigation`] interface which allows interacting
with the content, for example move with your mouse, entering keys with the
keyboard, etc... On top of that a `WebRTCDataChannel` based protocol has been
implemented and can be activated with the `enable-control-data-channel=true`
property allowing a client to send GstNavigation events using the WebRTC data channel.

The [gstwebrtc-api](gstwebrtc-api) and `webrtcsrc` implement the protocol as well
and they can be used as a client to control a remote sever.

You can easily test this feature using the [`wpesrc`] element with the following pipeline
that will start a server that allows you to navigate the GStreamer documentation:

``` shell
gst-launch-1.0 wpesrc location=https://gstreamer.freedesktop.org/documentation/ ! queue ! webrtcsink enable-control-data-channel=true meta="meta,name=web-stream"
```

You can control it inside the video running within your web browser (at
https://127.0.0.1:9090 if you followed previous steps in that readme) or
with the following GSteamer pipeline as a client:

``` shell
gst-launch-1.0 webrtcsrc signaller::producer-peer-id=<webrtcsink-peer-id> enable-control-data-channel=true ! videoconvert ! autovideosink
```

### Sending HTTP headers

During the initial signalling server handshake, you have the option to transmit 
HTTP headers, which can be utilized, for instance, for authentication purposes or sticky sessions:

``` shell
gst-launch-1.0 webrtcsink signaller::uri="ws://127.0.0.1:8443" signaller::headers="headers,foo=bar,cookie=\"session=1234567890; foo=bar\""
```

[`GstNavigation`]: https://gstreamer.freedesktop.org/documentation/video/gstnavigation.html
[`wpesrc`]: https://gstreamer.freedesktop.org/documentation/wpe/wpesrc.html

## Testing congestion control

For the purpose of testing congestion in a reproducible manner, a
[simple tool] has been used, it has been used on Linux exclusively but it is
also documented as usable on MacOS too. Client web browser has to be launched
on a separate machine on the LAN to test for congestion, although specific
configurations may allow to run it on the same machine.

Testing procedure was:

* identify the server machine network interface (e.g. with `ifconfig` on Linux)

* identify the client machine IP address (e.g. with `ifconfig` on Linux)

* start the various services as explained in the Usage section (use
  `GST_DEBUG=webrtcsink:7` to get detailed logs about congestion control)

* start playback in the client browser

* Run a `comcast` command on the server machine, for instance:

  ``` shell
  $HOME/go/bin/comcast --device=$SERVER_INTERFACE --target-bw 3000 --target-addr=$CLIENT_IP --target-port=1:65535 --target-proto=udp
  ```

* Observe the bitrate sharply decreasing, playback should slow down briefly
  then catch back up

* Remove the bandwidth limitation, and observe the bitrate eventually increasing
  back to a maximum:

  ``` shell
  $HOME/go/bin/comcast --device=$SERVER_INTERFACE --stop
  ```

For comparison, the congestion control property can be set to "disabled" on
webrtcsink, then the above procedure applied again, the expected result is
for playback to simply crawl down to a halt until the bandwidth limitation
is lifted:

``` shell
gst-launch-1.0 webrtcsink congestion-control=disabled
```

[simple tool]: https://github.com/tylertreat/comcast

## Monitoring tool

An example of client/server application for monitoring per-consumer stats
can be found [here].

[here]: https://gitlab.freedesktop.org/gstreamer/gst-plugins-rs/-/tree/main/net/webrtc/examples

## License

All the rust code in this repository is licensed under the
[Mozilla Public License Version 2.0].

Code in [gstwebrtc-api](gstwebrtc-api) is also licensed under the
[Mozilla Public License Version 2.0].

[Mozilla Public License Version 2.0]: http://opensource.org/licenses/MPL-2.0

## Control data channel

When the control data channel is enabled using the `enable-control-data-channel`
property, `webrtcsink` can optionally forward serialized metas and events to the
consumer.

Currently only timecode metas are supported, but other metas and events can easily
be added in a backward-compatible manner.

Here is an example for forwarding timecodes:

```
gst-launch-1.0 videotestsrc ! timecodestamper ! webrtcsink forward-metas="timecode" name=ws enable-control-data-channel=true run-signalling-server=true run-web-server=true
```

Point your web browser to `http://127.0.0.1:8080`, open the console and click
on the producer ID, you should see such messages in the console once the video
is displayed:

```
{
    "type": "InfoMessage",
    "mid": "video0",
    "info": {
        "meta": {
            "type": "timeCode",
            "hours": 0,
            "minutes": 0,
            "seconds": 5,
            "frames": 17,
            "field_count": 0,
            "fps": [
                30,
                1
            ],
            "flags": "none",
            "latest_daily_jam": "2024-08-23T17:51:42+0200"
        }
    }
}
```

## Using the AWS KVS signaller

You will need to build the crate with the `aws` feature enabled:

```
cargo build --features aws
```

* Setup AWS Kinesis Video Streams

* Create a channel from the AWS console (<https://us-east-1.console.aws.amazon.com/kinesisvideo/home?region=us-east-1#/signalingChannels/create>)

* Start a producer:

```
AWS_ACCESS_KEY_ID="XXX" AWS_SECRET_ACCESS_KEY="XXX" gst-launch-1.0 videotestsrc pattern=ball ! video/x-raw, width=1280, height=720 ! videoconvert ! textoverlay text="Hello from GStreamer!" ! videoconvert ! awskvswebrtcsink name=ws signaller::channel-name="XXX"
```

* Connect a viewer @ <https://awslabs.github.io/amazon-kinesis-video-streams-webrtc-sdk-js/examples/index.html>

## Using the WHIP Signaller

### WHIP Client

WHIP Client Signaller uses BaseWebRTCSink

Testing the whip client as the signaller can be done by setting up janus and
<https://github.com/meetecho/simple-whip-server/>.

* Set up a [janus] instance with the videoroom plugin configured
  to expose a room with ID 1234 (configuration in `janus.plugin.videoroom.jcfg`)

* Open the <janus/share/janus/demos/videoroomtest.html> web page, click start
  and join the room

* Set up the [simple whip server] as explained in its README

* Navigate to <http://localhost:7080/>, create an endpoint named room1234
  pointing to the Janus room with ID 1234

* Finally, send a stream to the endpoint with:

``` shell
gst-launch-1.0 -e uridecodebin uri=file:///home/meh/path/to/video/file ! \
  videoconvert ! video/x-raw ! queue ! \
  whipclientsink name=ws signaller::whip-endpoint="http://127.0.0.1:7080/whip/endpoint/room1234"
```

You should see a second video displayed in the videoroomtest web page.

### WHIP Server

WHIP Server Signaller uses BaseWebRTCSrc

The WHIP Server as the signaller can be tested in two ways.

Note: The initial version of `whipserversrc` does not check any auth or encryption.
Host application using `whipserversrc` behind an HTTP(s) proxy to enforce the auth and encryption between the WHIP client and server

#### 1. Using  the GStreamer element `whipclientsink`

a. In one tab of the terminal start the WHIP server using the below command

``` shell
RUST_BACKTRACE=full GST_DEBUG=webrtc*:6 GST_PLUGIN_PATH=target/x86_64-unknown-linux-gnu/debug:$GST_PLUGIN_PATH gst-launch-1.0 whipserversrc signaller::host-addr=http://127.0.0.1:8190 stun-server="stun://stun.l.google.com:19302" turn-servers="\<\"turns://user1:pass1@turn.serverone.com:7806\", \"turn://user2:pass2@turn.servertwo.com:7809\"\>" ! videoconvert ! autovideosink
```

b. In the second tab start the WHIP Client by sending a test video as shown in the below command

``` shell
RUST_BACKTRACE=full GST_DEBUG=webrtc*:6 GST_PLUGIN_PATH=target/x86_64-unknown-linux-gnu/debug:$GST_PLUGIN_PATH gst-launch-1.0 videotestsrc ! videoconvert ! video/x-raw ! queue ! \
  whipclientsink name=ws signaller::whip-endpoint="http://127.0.0.1:8190/whip/endpoint"
```

#### 2. Using Meetecho's `simple-whip-client`

Set up the simple whip client using using the instructions present in https://github.com/meetecho/simple-whip-client#readme

a. In one tab of the terminal start the WHIP server using the below command

``` shell
RUST_BACKTRACE=full GST_DEBUG=webrtc*:6 GST_PLUGIN_PATH=target/x86_64-unknown-linux-gnu/debug:$GST_PLUGIN_PATH gst-launch-1.0 whipserversrc signaller::host-addr=http://127.0.0.1:8190 stun-server="stun://stun.l.google.com:19302" turn-servers="\<\"turns://user1:pass1@turn.serverone.com:7806\", \"turn://user2:pass2@turn.servertwo.com:7809\"\>" name=ws ! videoconvert ! autovideosink ws. ! audioconvert ! autoaudiosink
```

b. In the second tab start the `simple-whip-client` as shown in the below command

``` shell
./whip-client --url http://127.0.0.1:8190/whip/endpoint \
        -A "audiotestsrc is-live=true wave=red-noise ! audioconvert ! audioresample ! queue ! opusenc perfect-timestamp=true ! rtpopuspay pt=100 ssrc=1 ! queue ! application/x-rtp,media=audio,encoding-name=OPUS,payload=100" \
        -V "videotestsrc is-live=true pattern=ball ! videoconvert ! queue ! vp8enc deadline=1 ! rtpvp8pay pt=96 ssrc=2 ! queue ! application/x-rtp,media=video,encoding-name=VP8,payload=96" \
        -S stun://stun.l.google.com:19302 \
        -l 7 \
        -n true
```

Terminating the client will close the session and the client should receive 200 (OK) as the response to the DELETE request

## Using the LiveKit Signaller

Testing the LiveKit signaller can be done by setting up [LiveKit] and creating a room.

You will need to build the crate with the `livekit` feature enabled:

```
cargo build --features livekit
```

You can connect either by given the API key and secret:

``` shell
gst-launch-1.0 -e uridecodebin uri=file:///home/meh/path/to/video/file ! \
  videoconvert ! video/x-raw ! queue ! \
  livekitwebrtcsink signaller::ws-url=ws://127.0.0.1:7880 signaller::api-key=devkey signaller::secret-key=secret signaller::room-name=testroom
```

Or by using a separately created authentication token
``` shell
gst-launch-1.0 -e uridecodebin uri=file:///home/meh/path/to/video/file ! \
  videoconvert ! video/x-raw ! queue ! \
  livekitwebrtcsink signaller::ws-url=ws://127.0.0.1:7880 signaller::auth-token=mygeneratedtoken signaller::room-name=testroom
```

Note: The generated auth token must have a grant of `{ "canSubscribe": false }`
or the server will disconnect `livekitwebrtcsink` after roughly 30 seconds.
This can be achieved with the `--grant` option to `livekit-cli` when generating
an auth token.

You should see a second video displayed in the videoroomtest web page.

## Streaming from LiveKit using the livekitwebrtcsrc element

First, publish a stream to the room using the following command:

```shell
gst-launch-1.0 livekitwebrtcsink name=sink \
    signaller::ws-url=ws://127.0.0.1:7880 \
    signaller::api-key=devkey \
    signaller::secret-key=secret \
    signaller::room-name=testroom \
    signaller::identity=gst-producer \
    signaller::participant-name=gst-producer \
    video-caps='video/x-h264' \
  videotestsrc is-live=1 \
  ! video/x-raw,width=640,height=360,framerate=15/1 \
  ! timeoverlay ! videoconvert ! queue ! sink.
```

Then play back the published stream:

```shell
gst-launch-1.0 livekitwebrtcsrc \
    name=src \
    signaller::ws-url=ws://127.0.0.1:7880 \
    signaller::api-key=devkey \
    signaller::secret-key=secret \
    signaller::room-name=testroom \
    signaller::identity=gst-consumer \
    signaller::participant-name=gst-consumer \
    signaller::producer-peer-id=gst-producer \
    video-codecs='<H264>' \
  src. ! queue ! videoconvert ! autovideosink
```

### Auto-subscribe with livekitwebrtcsrc element

With the LiveKit source element, you can also subscribe to all the peers in
your room, simply by not specifying any value for
`signaller::producer-peer-id`. Unwanted peers can also be ignored by supplying
an array of peer IDs to `signaller::excluded-producer-peer-ids`. Importantly,
it is also necessary to add sinks for all the streams in the room that the
source element has subscribed to.

First, publish a few streams using different connections:

```shell
gst-launch-1.0 \
  livekitwebrtcsink name=sinka \
    signaller::ws-url=ws://127.0.0.1:7880 \
    signaller::api-key=devkey \
    signaller::secret-key=secret \
    signaller::room-name=testroom \
    signaller::identity=gst-producer-a \
    signaller::participant-name=gst-producer-a \
    video-caps='video/x-vp8' \
  livekitwebrtcsink name=sinkb \
    signaller::ws-url=ws://127.0.0.1:7880 \
    signaller::api-key=devkey \
    signaller::secret-key=secret \
    signaller::room-name=testroom \
    signaller::identity=gst-producer-b \
    signaller::participant-name=gst-producer-b \
    video-caps='video/x-vp8' \
  livekitwebrtcsink name=sinkc \
    signaller::ws-url=ws://127.0.0.1:7880 \
    signaller::api-key=devkey \
    signaller::secret-key=secret \
    signaller::room-name=testroom \
    signaller::identity=gst-producer-c \
    signaller::participant-name=gst-producer-c \
    video-caps='video/x-vp8' \
  videotestsrc is-live=1 \
  ! video/x-raw,width=640,height=360,framerate=15/1 \
  ! timeoverlay ! videoconvert ! queue ! sinka. \
  videotestsrc pattern=ball is-live=1 \
  ! video/x-raw,width=320,height=180,framerate=15/1 \
  ! timeoverlay ! videoconvert ! queue ! sinkb.
  videotestsrc is-live=1 \
  ! video/x-raw,width=320,height=180,framerate=15/1 \
  ! timeoverlay ! videoconvert ! queue ! sinkc.
```

Then watch only streams A and B by excluding peer C:

```shell
gst-launch-1.0 livekitwebrtcsrc \
  name=src \
  signaller::ws-url=ws://127.0.0.1:7880 \
  signaller::api-key=devkey \
  signaller::secret-key=secret \
  signaller::room-name=testroom \
  signaller::identity=gst-consumer \
  signaller::participant-name=gst-consumer \
  signaller::excluded-producer-peer-ids='<gst-producer-c>' \
  src. ! queue ! videoconvert ! autovideosink
  src. ! queue ! videoconvert ! autovideosink
```

[LiveKit]: https://livekit.io/
[janus]: https://github.com/meetecho/janus-gateway
[simple whip server]: https://github.com/meetecho/simple-whip-server/
