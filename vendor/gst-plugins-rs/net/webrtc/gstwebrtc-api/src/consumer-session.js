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

import WebRTCSession from "./webrtc-session.js";
import SessionState from "./session-state.js";
import RemoteController from "./remote-controller.js";

/**
 * Event name: "streamsChanged".<br>
 * Triggered when the underlying media streams of a {@link ConsumerSession} change.
 * @event GstWebRTCAPI#StreamsChangedEvent
 * @type {Event}
 * @see ConsumerSession#streams
 */
/**
 * Event name: "remoteControllerChanged".<br>
 * Triggered when the underlying remote controller of a {@link ConsumerSession} changes.
 * @event GstWebRTCAPI#RemoteControllerChangedEvent
 * @type {Event}
 * @see ConsumerSession#remoteController
 */

/**
 * @class ConsumerSession
 * @hideconstructor
 * @classdesc Consumer session managing a peer-to-peer WebRTC channel between a remote producer and this client
 * instance.
 * <p>Call {@link GstWebRTCAPI#createConsumerSession} to create a ConsumerSession instance.</p>
 * @extends {WebRTCSession}
 * @fires {@link GstWebRTCAPI#event:StreamsChangedEvent}
 * @fires {@link GstWebRTCAPI#event:RemoteControllerChangedEvent}
 */
class ConsumerSession extends WebRTCSession {
  constructor(peerId, comChannel, offerOptions) {
    super(peerId, comChannel);
    this._streams = [];
    this._remoteController = null;
    this._pendingCandidates = [];
    this._mungeStereoHack = false;

    this._offerOptions = offerOptions;

    this.addEventListener("closed", () => {
      this._streams = [];
      this._pendingCandidates = [];

      if (this._remoteController) {
        this._remoteController.close();
      }
    });
  }

  /**
   * Defines whether the SDP should be munged in order to enable stereo with chrome.
   * @method
   * @param {boolean} enable - Enable or disable the hack, default is false
   */
  set mungeStereoHack(enable) {
    if (typeof (enable) !== "boolean") { return; }

    this._mungeStereoHack = enable;
  }

  /**
   * The array of remote media streams consumed locally through this WebRTC channel.
   * @type {MediaStream[]}
   * @readonly
   */
  get streams() {
    return this._streams;
  }

  /**
   * The remote controller associated with this WebRTC consumer session. Value may be null if consumer session
   * has no remote controller.
   * @type {RemoteController}
   * @readonly
   */
  get remoteController() {
    return this._remoteController;
  }

  /**
   * Connects the consumer session to its remote producer.<br>
   * This method must be called after creating the consumer session in order to start receiving the remote streams.
   * It registers this consumer session to the signaling server and gets ready to receive audio/video streams.
   * <p>Even on success, streaming can fail later if any error occurs during or after connection. In order to know
   * the effective streaming state, you should be listening to the [error]{@link GstWebRTCAPI#event:ErrorEvent},
   * [stateChanged]{@link GstWebRTCAPI#event:StateChangedEvent} and/or [closed]{@link GstWebRTCAPI#event:ClosedEvent}
   * events.</p>
   * @returns {boolean} true in case of success (may fail later during or after connection) or false in case of
   * immediate error (wrong session state or no connection to the signaling server).
   */
  connect() {
    if (!this._comChannel || (this._state === SessionState.closed)) {
      return false;
    }

    if (this._state !== SessionState.idle) {
      return true;
    }

    if (this._offerOptions) {
      this.ensurePeerConnection();

      this._rtcPeerConnection.createDataChannel("control");

      this._rtcPeerConnection.createOffer(this._offerOptions).then((desc) => {
        if (this._rtcPeerConnection && desc) {
          return this._rtcPeerConnection.setLocalDescription(desc);
        } else {
          throw new Error("cannot send local offer to WebRTC peer");
        }
      }).then(() => {
        if (this._rtcPeerConnection && this._comChannel) {
          const msg = {
            type: "startSession",
            peerId: this._peerId,
            offer: this._rtcPeerConnection.localDescription.toJSON().sdp
          };
          if (!this._comChannel.send(msg)) {
            throw new Error("cannot send startSession message to signaling server");
          }
          this._state = SessionState.connecting;
          this.dispatchEvent(new Event("stateChanged"));
        }
      }).catch((ex) => {
        if (this._state !== SessionState.closed) {
          this.dispatchEvent(new ErrorEvent("error", {
            message: "an unrecoverable error occurred during SDP handshake",
            error: ex
          }));

          this.close();
        }
      });
    } else {
      const msg = {
        type: "startSession",
        peerId: this._peerId
      };
      if (!this._comChannel.send(msg)) {
        this.dispatchEvent(new ErrorEvent("error", {
          message: "cannot connect consumer session",
          error: new Error("cannot send startSession message to signaling server")
        }));

        this.close();
        return false;
      }

      this._state = SessionState.connecting;
      this.dispatchEvent(new Event("stateChanged"));
    }

    return true;
  }

  onSessionStarted(peerId, sessionId) {
    if ((this._peerId === peerId) && (this._state === SessionState.connecting) && !this._sessionId) {
      console.log("Session started", this._sessionId);
      this._sessionId = sessionId;

      for (const candidate of this._pendingCandidates) {
        console.log("Sending delayed ICE with session id", this._sessionId);
        this._comChannel.send({
          type: "peer",
          sessionId: this._sessionId,
          ice: candidate.toJSON()
        });
      }

      this._pendingCandidates = [];
    }
  }

  ensurePeerConnection() {
    if (!this._rtcPeerConnection) {
      const connection = new RTCPeerConnection(this._comChannel.webrtcConfig);
      this._rtcPeerConnection = connection;

      connection.ontrack = (event) => {
        if ((this._rtcPeerConnection === connection) && event.streams && (event.streams.length > 0)) {
          if (this._state === SessionState.connecting) {
            this._state = SessionState.streaming;
            this.dispatchEvent(new Event("stateChanged"));
          }

          let streamsChanged = false;
          for (const stream of event.streams) {
            if (!this._streams.includes(stream)) {
              this._streams.push(stream);
              streamsChanged = true;
            }
          }

          if (streamsChanged) {
            this.dispatchEvent(new Event("streamsChanged"));
          }
        }
      };

      connection.ondatachannel = (event) => {
        const rtcDataChannel = event.channel;
        if (rtcDataChannel && (rtcDataChannel.label === "control")) {
          if (this._remoteController) {
            const previousController = this._remoteController;
            this._remoteController = null;
            previousController.close();
          }

          const remoteController = new RemoteController(rtcDataChannel, this);
          this._remoteController = remoteController;
          this.dispatchEvent(new Event("remoteControllerChanged"));

          remoteController.addEventListener("closed", () => {
            if (this._remoteController === remoteController) {
              this._remoteController = null;
              this.dispatchEvent(new Event("remoteControllerChanged"));
            }
          });
        }
      };

      connection.onicecandidate = (event) => {
        if ((this._rtcPeerConnection === connection) && event.candidate && this._comChannel) {
          if (this._sessionId) {
            console.log("Sending ICE with session id", this._sessionId);
            this._comChannel.send({
              type: "peer",
              sessionId: this._sessionId,
              ice: event.candidate.toJSON()
            });
          } else {
            this._pendingCandidates.push(event.candidate);
          }
        }
      };

      this.dispatchEvent(new Event("rtcPeerConnectionChanged"));
    }
  }

  // Work around Chrome not handling stereo Opus correctly.
  // See
  //   https://chromium.googlesource.com/external/webrtc/+/194e3bcc53ffa3e98045934377726cb25d7579d2/webrtc/media/engine/webrtcvoiceengine.cc#302
  //   https://bugs.chromium.org/p/webrtc/issues/detail?id=8133
  //
  // Technically it's against the spec to modify the SDP
  // but there's no other API for this and this seems to
  // be the only possible workaround at this time.
  mungeStereo(offerSdp, answerSdp) {
    const stereoRegexp = /a=fmtp:.* sprop-stereo/g;
    let stereoPayloads = new Set();
    for (const m of offerSdp.matchAll(stereoRegexp)) {
      const payloadMatch = m[0].match(/a=fmtp:(\d+) .*/);
      if (payloadMatch) {
        stereoPayloads.add(payloadMatch[1]);
      }
    }

    for (const payload of stereoPayloads) {
      const isStereoRegexp = new RegExp("a=fmtp:" + payload + ".*stereo");
      const answerIsStereo = answerSdp.match(isStereoRegexp);

      if (!answerIsStereo) {
        answerSdp = answerSdp.replaceAll("a=fmtp:" + payload, "a=fmtp:" + payload + " stereo=1;");
      }
    }

    return answerSdp;
  }

  onSessionPeerMessage(msg) {
    if ((this._state === SessionState.closed) || !this._comChannel || !this._sessionId) {
      return;
    }

    this.ensurePeerConnection();

    if (msg.sdp) {
      if (this._offerOptions) {
        this._rtcPeerConnection.setRemoteDescription(msg.sdp).then(() => {
          console.log("done");
        }).catch((ex) => {
          if (this._state !== SessionState.closed) {
            this.dispatchEvent(new ErrorEvent("error", {
              message: "an unrecoverable error occurred during SDP handshake",
              error: ex
            }));

            this.close();
          }
        });
      } else {
        this._rtcPeerConnection.setRemoteDescription(msg.sdp).then(() => {
          if (this._rtcPeerConnection) {
            return this._rtcPeerConnection.createAnswer();
          } else {
            return null;
          }
        }).then((desc) => {
          if (this._rtcPeerConnection && desc) {
            if (this._mungeStereoHack) {
              desc.sdp = this.mungeStereo(msg.sdp.sdp, desc.sdp);
            }

            return this._rtcPeerConnection.setLocalDescription(desc);
          } else {
            return null;
          }
        }).then(() => {
          if (this._rtcPeerConnection && this._comChannel) {
            console.log("Sending SDP with session id", this._sessionId);
            const sdp = {
              type: "peer",
              sessionId: this._sessionId,
              sdp: this._rtcPeerConnection.localDescription.toJSON()
            };
            if (!this._comChannel.send(sdp)) {
              throw new Error("cannot send local SDP configuration to WebRTC peer");
            }
          }
        }).catch((ex) => {
          if (this._state !== SessionState.closed) {
            this.dispatchEvent(new ErrorEvent("error", {
              message: "an unrecoverable error occurred during SDP handshake",
              error: ex
            }));

            this.close();
          }
        });
      }
    } else if (msg.ice) {
      const candidate = msg.ice.candidate ? new RTCIceCandidate(msg.ice) : null;
      this._rtcPeerConnection.addIceCandidate(candidate).catch((ex) => {
        if (this._state !== SessionState.closed) {
          this.dispatchEvent(new ErrorEvent("error", {
            message: "an unrecoverable error occurred during ICE handshake",
            error: ex
          }));

          this.close();
        }
      });
    } else {
      throw new Error(`invalid empty peer message received from consumer session ${this._sessionId}`);
    }
  }
}

export default ConsumerSession;
