/*
 * gstwebrtc-api
 *
 * Copyright (C) 2022 Igalia S.L. <info@igalia.com>
 *   Author: Loïc Le Page <llepage@igalia.com>
 *
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

/**
 * GStreamer WebRTC configuration.
 * <p>The config can be passed on creation of the GstWebRTCAPI class.</p>
 * <p>For example:
 * <pre>
 *     const signalingProtocol = window.location.protocol.startsWith("https") ? "wss" : "ws";
 *     const api = new GstWebRTCAPI({
 *         meta: { name: `WebClient-${Date.now()}` },
 *         signalingServerUrl: `${signalingProtocol}://${window.location.host}/webrtc`
 *     });
 * </pre></p>
 * @typedef {object} GstWebRTCConfig
 * @property {object} meta=null - Client free-form information that will be exchanged with all peers through the
 * signaling <i>meta</i> property, its content depends on your application.
 * @property {string} signalingServerUrl=ws://127.0.0.1:8443 - The WebRTC signaling server URL.
 * @property {number} reconnectionTimeout=2500 - Timeout in milliseconds to reconnect to the signaling server in
 * case of an unexpected disconnection.
 * @property {object} webrtcConfig={iceServers...} - The WebRTC peer connection configuration passed to
 * {@link RTCPeerConnection}. Default configuration only includes a list of free STUN servers
 * (<i>stun[0-4].l.google.com:19302</i>).
 */

/**
 * Default GstWebRTCAPI configuration.
 * @type {GstWebRTCConfig}
 */
const defaultConfig = Object.freeze({
  meta: null,
  signalingServerUrl: "ws://127.0.0.1:8443",
  reconnectionTimeout: 2500,
  webrtcConfig: {
    iceServers: [
      {
        urls: [
          "stun:stun.l.google.com:19302",
          "stun:stun1.l.google.com:19302"
        ]
      }
    ],
    bundlePolicy: "max-bundle"
  }
});

export default defaultConfig;
