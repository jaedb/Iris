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

import SessionState from "./session-state.js";

/** @import ProducerSession from "./producer-session.js"; */
/** @import ClientSession from "./producer-session.js"; */

/**
 * Event name: "error".<br>
 * Triggered when any kind of error occurs.
 * <p>When emitted by a session, it is in general an unrecoverable error. Normally, the session is automatically closed
 * but in the specific case of a {@link ProducerSession}, when the error occurs on an underlying
 * {@link ClientSession} between the producer session and a remote client consuming the streamed media,
 * then only the failing {@link ClientSession} is closed. The producer session can keep on serving the
 * other consumer peers.</p>
 * @event GstWebRTCAPI#ErrorEvent
 * @type {ErrorEvent}
 * @property {string} message - The error message.
 * @property {Error} error - The error exception.
 * @see WebRTCSession
 * @see RemoteController
 */
/**
 * Event name: "stateChanged".<br>
 * Triggered each time a session state changes.
 * @event GstWebRTCAPI#StateChangedEvent
 * @type {Event}
 * @see WebRTCSession#state
 */
/**
 * Event name: "rtcPeerConnectionChanged".<br>
 * Triggered each time a session internal {@link RTCPeerConnection} changes. This can occur during the session
 * connecting state when the peer-to-peer WebRTC connection is established, and when closing the
 * {@link WebRTCSession}.
 * @event GstWebRTCAPI#RTCPeerConnectionChangedEvent
 * @type {Event}
 * @see WebRTCSession#rtcPeerConnection
 */
/**
 * Event name: "closed".<br>
 * Triggered when a session is definitively closed (then it can be garbage collected as session instances are not
 * reusable).
 * @event GstWebRTCAPI#ClosedEvent
 * @type {Event}
 */

/**
 * @class WebRTCSession
 * @hideconstructor
 * @classdesc Manages a WebRTC session between a producer and a consumer (peer-to-peer channel).
 * @extends {EventTarget}
 * @fires {@link GstWebRTCAPI#event:ErrorEvent}
 * @fires {@link GstWebRTCAPI#event:StateChangedEvent}
 * @fires {@link GstWebRTCAPI#event:RTCPeerConnectionChangedEvent}
 * @fires {@link GstWebRTCAPI#event:ClosedEvent}
 * @see ConsumerSession
 * @see ProducerSession
 * @see ClientSession
 */
class WebRTCSession extends EventTarget {
  constructor(peerId, comChannel) {
    super();

    this._peerId = peerId;
    this._sessionId = "";
    this._comChannel = comChannel;
    this._state = SessionState.idle;
    this._rtcPeerConnection = null;
  }

  /**
   * Unique identifier of the remote peer to which this session is connected.
   * @type {string}
   * @readonly
   */
  get peerId() {
    return this._peerId;
  }

  /**
   * Unique identifier of this session (defined by the signaling server).<br>
   * The local session ID equals "" until it is created on server side. This is done during the connection handshake.
   * The local session ID is guaranteed to be valid and to correctly reflect the signaling server value once
   * session state has switched to {@link SessionState#streaming}.
   * @type {string}
   * @readonly
   */
  get sessionId() {
    return this._sessionId;
  }

  /**
   * The current WebRTC session state.
   * @type {SessionState}
   * @readonly
   */
  get state() {
    return this._state;
  }

  /**
   * The internal {@link RTCPeerConnection} used to manage the underlying WebRTC connection with session
   * peer. Value may be null if session has no active WebRTC connection. You can listen to the
   * {@link GstWebRTCAPI#event:RTCPeerConnectionChangedEvent} event to be informed when the connection is established
   * or destroyed.
   * @type {RTCPeerConnection}
   * @readonly
   */
  get rtcPeerConnection() {
    return this._rtcPeerConnection;
  }

  /**
   * Terminates the WebRTC session.<br>
   * It immediately disconnects the remote peer attached to this session and unregisters the session from the
   * signaling server.
   */
  close() {
    if (this._state !== SessionState.closed) {
      if ((this._state !== SessionState.idle) && this._comChannel && this._sessionId) {
        this._comChannel.send({
          type: "endSession",
          sessionId: this._sessionId
        });
      }

      this._state = SessionState.closed;
      this.dispatchEvent(new Event("stateChanged"));

      this._comChannel = null;

      if (this._rtcPeerConnection) {
        this._rtcPeerConnection.close();
        this._rtcPeerConnection = null;
        this.dispatchEvent(new Event("rtcPeerConnectionChanged"));
      }

      this.dispatchEvent(new Event("closed"));
    }
  }
}

export default WebRTCSession;
