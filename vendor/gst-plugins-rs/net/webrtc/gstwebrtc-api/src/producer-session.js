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

/**
 * @class ClientSession
 * @hideconstructor
 * @classdesc Client session representing a link between a remote consumer and a local producer session.
 * @extends {WebRTCSession}
 */
class ClientSession extends WebRTCSession {
  constructor(peerId, sessionId, comChannel, stream) {
    super(peerId, comChannel);
    this._sessionId = sessionId;
    this._state = SessionState.streaming;

    const connection = new RTCPeerConnection(this._comChannel.webrtcConfig);
    this._rtcPeerConnection = connection;

    for (const track of stream.getTracks()) {
      connection.addTrack(track, stream);
    }

    connection.onicecandidate = (event) => {
      if ((this._rtcPeerConnection === connection) && event.candidate && this._comChannel) {
        this._comChannel.send({
          type: "peer",
          sessionId: this._sessionId,
          ice: event.candidate.toJSON()
        });
      }
    };

    this.dispatchEvent(new Event("rtcPeerConnectionChanged"));

    connection.setLocalDescription().then(() => {
      if ((this._rtcPeerConnection === connection) && this._comChannel) {
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

  onSessionPeerMessage(msg) {
    if ((this._state === SessionState.closed) || !this._rtcPeerConnection) {
      return;
    }

    if (msg.sdp) {
      this._rtcPeerConnection.setRemoteDescription(msg.sdp).catch((ex) => {
        if (this._state !== SessionState.closed) {
          this.dispatchEvent(new ErrorEvent("error", {
            message: "an unrecoverable error occurred during SDP handshake",
            error: ex
          }));

          this.close();
        }
      });
    } else if (msg.ice) {
      const candidate = new RTCIceCandidate(msg.ice);
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
      throw new Error(`invalid empty peer message received from producer's client session ${this._peerId}`);
    }
  }
}

/**
 * Event name: "clientConsumerAdded".<br>
 * Triggered when a remote consumer peer connects to a local {@link ProducerSession}.
 * @event GstWebRTCAPI#ClientConsumerAddedEvent
 * @type {CustomEvent}
 * @property {ClientSession} detail - The WebRTC session associated with the added consumer peer.
 * @see ProducerSession
 */
/**
 * Event name: "clientConsumerRemoved".<br>
 * Triggered when a remote consumer peer disconnects from a local {@link ProducerSession}.
 * @event GstWebRTCAPI#ClientConsumerRemovedEvent
 * @type {CustomEvent}
 * @property {ClientSession} detail - The WebRTC session associated with the removed consumer peer.
 * @see ProducerSession
 */

/**
 * @class ProducerSession
 * @hideconstructor
 * @classdesc Producer session managing the streaming out of a local {@link MediaStream}.<br>
 * It manages all underlying WebRTC connections to each peer client consuming the stream.
 * <p>Call {@link GstWebRTCAPI#createProducerSession} to create a ProducerSession instance.</p>
 * @extends {EventTarget}
 * @fires {@link GstWebRTCAPI#event:ErrorEvent}
 * @fires {@link GstWebRTCAPI#event:StateChangedEvent}
 * @fires {@link GstWebRTCAPI#event:ClosedEvent}
 * @fires {@link GstWebRTCAPI#event:ClientConsumerAddedEvent}
 * @fires {@link GstWebRTCAPI#event:ClientConsumerRemovedEvent}
 */
class ProducerSession extends EventTarget {
  constructor(comChannel, stream, consumerId) {
    super();

    this._comChannel = comChannel;
    this._stream = stream;
    this._state = SessionState.idle;
    this._clientSessions = {};
    this._consumerId = consumerId;
  }

  /**
   * The local stream produced out by this session.
   * @type {MediaStream}
   * @readonly
   */
  get stream() {
    return this._stream;
  }

  /**
   * The current producer session state.
   * @type {SessionState}
   * @readonly
   */
  get state() {
    return this._state;
  }

  /**
   * Starts the producer session.<br>
   * This method must be called after creating the producer session in order to start streaming. It registers this
   * producer session to the signaling server and gets ready to serve peer requests from consumers.
   * <p>Even on success, streaming can fail later if any error occurs during or after connection. In order to know
   * the effective streaming state, you should be listening to the [error]{@link GstWebRTCAPI#event:ErrorEvent},
   * [stateChanged]{@link GstWebRTCAPI#event:StateChangedEvent} and/or [closed]{@link GstWebRTCAPI#event:ClosedEvent}
   * events.</p>
   * @returns {boolean} true in case of success (may fail later during or after connection) or false in case of
   * immediate error (wrong session state or no connection to the signaling server).
   */
  start() {
    if (!this._comChannel || (this._state === SessionState.closed)) {
      return false;
    }

    if (this._state !== SessionState.idle) {
      return true;
    }

    const msg = {
      type: "setPeerStatus",
      roles: ["listener", "producer"],
      meta: this._comChannel.meta
    };
    if (!this._comChannel.send(msg)) {
      this.dispatchEvent(new ErrorEvent("error", {
        message: "cannot start producer session",
        error: new Error("cannot register producer to signaling server")
      }));

      this.close();
      return false;
    }

    this._state = SessionState.connecting;
    this.dispatchEvent(new Event("stateChanged"));
    return true;
  }

  /**
   * Terminates the producer session.<br>
   * It immediately disconnects all peer consumers attached to this producer session and unregisters the producer
   * from the signaling server.
   */
  close() {
    if (this._state !== SessionState.closed) {
      for (const track of this._stream.getTracks()) {
        track.stop();
      }

      if ((this._state !== SessionState.idle) && this._comChannel) {
        this._comChannel.send({
          type: "setPeerStatus",
          roles: ["listener"],
          meta: this._comChannel.meta
        });
      }

      this._state = SessionState.closed;
      this.dispatchEvent(new Event("stateChanged"));

      this._comChannel = null;
      this._stream = null;

      for (const clientSession of Object.values(this._clientSessions)) {
        clientSession.close();
      }
      this._clientSessions = {};

      this.dispatchEvent(new Event("closed"));
    }
  }

  onProducerRegistered() {
    if (this._state === SessionState.connecting) {
      this._state = SessionState.streaming;
      this.dispatchEvent(new Event("stateChanged"));
    }

    if (this._consumerId) {
      const msg = {
        type: "startSession",
        peerId: this._consumerId
      };
      if (!this._comChannel.send(msg)) {
        this.dispatchEvent(new ErrorEvent("error", {
          message: "cannot send session request to specified consumer",
          error: new Error("cannot send startSession message to signaling server")
        }));

        this.close();
      }
    }
  }

  onStartSessionMessage(msg) {
    if (this._comChannel && this._stream && !(msg.sessionId in this._clientSessions)) {
      const session = new ClientSession(msg.peerId, msg.sessionId, this._comChannel, this._stream);
      this._clientSessions[msg.sessionId] = session;

      session.addEventListener("closed", (event) => {
        const sessionId = event.target.sessionId;
        if ((sessionId in this._clientSessions) && (this._clientSessions[sessionId] === session)) {
          delete this._clientSessions[sessionId];
          this.dispatchEvent(new CustomEvent("clientConsumerRemoved", { detail: session }));
        }
      });

      session.addEventListener("error", (event) => {
        this.dispatchEvent(new ErrorEvent("error", {
          message: `error from client consumer ${event.target.peerId}: ${event.message}`,
          error: event.error
        }));
      });

      this.dispatchEvent(new CustomEvent("clientConsumerAdded", { detail: session }));
    }
  }

  onEndSessionMessage(msg) {
    if (msg.sessionId in this._clientSessions) {
      this._clientSessions[msg.sessionId].close();
    }
  }

  onSessionPeerMessage(msg) {
    if (msg.sessionId in this._clientSessions) {
      this._clientSessions[msg.sessionId].onSessionPeerMessage(msg);
    }
  }
}

export default ProducerSession;
