# gstwebrtc-api

[![License: MPL 2.0](https://img.shields.io/badge/License-MPL_2.0-brightgreen.svg)](https://opensource.org/licenses/MPL-2.0)

Javascript API used to integrate GStreamer WebRTC streams produced and consumed by webrtcsink and webrtcsrc elements
into a web browser or a mobile WebView.

This API allows a complete 360º interconnection between GStreamer and web interfaces for realtime streaming using the
WebRTC protocol.

This API is released under the Mozilla Public License Version 2.0 (MPL-2.0) that can be found in the LICENSE-MPL-2.0
file or at https://opensource.org/licenses/MPL-2.0

Copyright (C) 2022 Igalia S.L. <<info@igalia.com>><br>
Author: Loïc Le Page <<llepage@igalia.com>>

It includes external source code from [webrtc-adapter](https://github.com/webrtcHacks/adapter) that is embedded with
the API. The webrtc-adapter BSD 3-Clause license is available at
https://github.com/webrtcHacks/adapter/blob/master/LICENSE.md

Webrtc-adapter is Copyright (c) 2014, The WebRTC project authors, All rights reserved.<br>
Copyright (c) 2018, The adapter.js project authors, All rights reserved.

## Building the API

The GstWebRTC API uses [Webpack](https://webpack.js.org/) to bundle all source files and dependencies together.

You only need to install [Node.js](https://nodejs.org/en/) to run all commands.

On first time, install the dependencies by calling:
```shell
$ npm install
```

Then build the bundle by calling:
```shell
$ npm run make
```

It will build and compress the code into the *dist/* folder, there you will find the following files:
- *gstwebrtc-api-[version].min.js* which is the only file you need to include in your web application to use the API,
  if you are manually adding the .js file to your webpage. It already embeds all dependencies.
- *gstwebrtc-api-[version].min.js.map* which is useful for debugging the API code, you need to put it in the same
  folder as the API script on your web server if you want to allow debugging, else you can just ignore it.
- *gstwebrtc-api-[version].esm.js, which is the ES module variant of this library. You probably don't need to use
  this directly - it will be automatically used by your build system if you install this package via npm. See below.
- *gstwebrtc-api-[version].esm.js.map* which is the source map for the ES module variant mentioned above.

The API documentation is created into the *docs/* folder. It is automatically created when building the whole API.

If you want to build the documentation only, you can call:
```shell
$ npm run docs
```

If you only want to build the API without the documentation, you can call:
```shell
$ npm run build
```

## Packaging the API

You can create a portable package of the API by calling:
```shell
$ npm pack
```

It will create a *gstwebrtc-api-[version].tgz* file that contains all source code, documentation and built API. This
portable package can be installed as a dependency in any Node.js project by calling:
```shell
$ npm install gstwebrtc-api-[version].tgz
```

## Testing and debugging the API

To easily test and debug the GstWebRTC API, you just need to:
1. launch the webrtc signalling server by calling (from the repository *gst-plugins-rs* root folder):
   ```shell
   $ cargo run --bin gst-webrtc-signalling-server
   ```
2. launch the GstWebRTC API server by calling (from the *net/webrtc/gstwebrtc-api* sub-folder):
   ```shell
   $ npm start
   ```

It will launch a local HTTPS server listening on port 9090 and using an automatically generated self-signed
certificate.

With this server you can test the reference example shipped in *index.html* from a web browser on your local computer
or a mobile device.

## Interconnect with GStreamer pipelines

Once the signalling and gstwebrtc-api servers launched, you can interconnect the streams produced and consumed from
the web browser with GStreamer pipelines using the webrtcsink and webrtcsrc elements.

### Consume a WebRTC stream produced by the gstwebrtc-api

On the web browser side, click on the *Start Capture* button and give access to the webcam. The gstwebrtc-api will
start producing a video stream.

The signalling server logs will show the registration of a new producer with the same *peer_id* as the *Client ID*
that appears on the webpage.

Then launch the following GStreamer pipeline:
```shell
$ gst-launch-1.0 playbin3 uri=gstwebrtc://[signalling server]?peer-id=[client ID of the producer]
```

Using the local signalling server, it will look like this:
```shell
$ gst-launch-1.0 playbin3 uri=gstwebrtc://127.0.0.1:8443?peer-id=e54e5d6b-f597-4e8f-bc96-2cc3765b6567
```

The underlying *uridecodebin* element recognizes the *gstwebrtc://* scheme as a WebRTC stream compatible with the
gstwebrtc-api and will correctly use a *webrtcsrc* element to manage this stream.

The *gstwebrtc://* scheme is used for normal WebSocket connections to the signalling server, and the *gstwebrtcs://*
scheme for secured connections over SSL or TLS.

### Produce a GStreamer WebRTC stream consumed by the gstwebrtc-api

Launch the following GStreamer pipeline:
```shell
$ gst-launch-1.0 videotestsrc ! agingtv ! webrtcsink meta="meta,name=native-stream"
```

By default *webrtcsink* element uses *ws://127.0.0.1:8443* for the signalling server address, so there is no need
for more arguments. If you're hosting the signalling server elsewhere, you can specify its address by adding
`signaller::uri="ws[s]://[signalling server]"` to the list of *webrtcsink* properties.

Once the GStreamer pipeline launched, you will see the registration of a new producer in the logs of the signalling
server and a new remote stream, with the name *native-stream*, will appear on the webpage.

You just need to click on the corresponding entry to connect as a consumer to the remote native stream.

### Produce a GStreamer interactive WebRTC stream with remote control

Launch the following GStreamer pipeline:
```shell
$ gst-launch-1.0 wpesrc location=https://gstreamer.freedesktop.org/documentation ! queue ! webrtcsink enable-control-data-channel=true meta="meta,name=web-stream"
```

Once the GStreamer pipeline launched, you will see a new producer with the name *web-stream*. When connecting to this
producer you will see the remote rendering of the web page. You can interact remotely with this web page, controls are
sent through a special WebRTC data channel while the rendering is done remotely by the
[wpesrc](https://gstreamer.freedesktop.org/documentation/wpe/wpesrc.html) element.
