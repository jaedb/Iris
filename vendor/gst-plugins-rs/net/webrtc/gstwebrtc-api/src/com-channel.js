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

import ConsumerSession from "./consumer-session.js";
import ProducerSession from "./producer-session.js";

const SignallingServerMessageType = Object.freeze({
  welcome: "welcome",
  peerStatusChanged: "peerStatusChanged",
  list: "list",
  listConsumers: "listConsumers",
  sessionStarted: "sessionStarted",
  peer: "peer",
  startSession: "startSession",
  endSession: "endSession",
  error: "error"
});

function normalizePeer(peer, excludedId) {
  if (!peer || (typeof (peer) !== "object")) {
    return null;
  }

  const normalizedPeer = {
    id: "",
    meta: {}
  };

  if (peer.id && (typeof (peer.id) === "string")) {
    normalizedPeer.id = peer.id;
  } else if (peer.peerId && (typeof (peer.peerId) === "string")) {
    normalizedPeer.id = peer.peerId;
  } else {
    return null;
  }

  if (normalizedPeer.id === excludedId) {
    return null;
  }

  if (peer.meta && (typeof (peer.meta) === "object")) {
    normalizedPeer.meta = peer.meta;
  }

  Object.freeze(normalizedPeer.meta);
  return Object.freeze(normalizedPeer);
}

class ComChannel extends EventTarget {
  constructor(url, meta, webrtcConfig) {
    super();

    this._meta = meta;
    this._webrtcConfig = webrtcConfig;
    this._ws = new WebSocket(url);
    this._ready = false;
    this._channelId = "";
    this._producerSession = null;
    this._consumerSessions = {};
    this._peers = {};

    this._ws.onerror = (event) => {
      this.dispatchEvent(new ErrorEvent("error", {
        message: event.message || "WebSocket error",
        error: event.error || new Error(
          this._ready ? "transportation error" : "cannot connect to signaling server")
      }));
      this.close();
    };

    this._ws.onclose = () => {
      this._ready = false;
      this._channelId = "";
      this._ws = null;

      this.closeAllConsumerSessions();

      if (this._producerSession) {
        this._producerSession.close();
        this._producerSession = null;
      }

      this.dispatchEvent(new Event("closed"));
    };

    this._ws.onmessage = (event) => {
      try {
        const msg = JSON.parse(event.data);
        if (msg && (typeof (msg) === "object")) {
          switch (msg.type) {

          case SignallingServerMessageType.welcome:
            this._channelId = msg.peerId;
            try {
              this._ws.send(JSON.stringify({
                type: "setPeerStatus",
                roles: ["listener"],
                meta: meta
              }));
            } catch (ex) {
              this.dispatchEvent(new ErrorEvent("error", {
                message: "cannot initialize connection to signaling server",
                error: ex
              }));
              this.close();
            }
            break;

          case SignallingServerMessageType.peerStatusChanged: {
            if (msg.peerId === this._channelId) {
              if (!this._ready && msg.roles.includes("listener")) {
                this._ready = true;
                this.dispatchEvent(new Event("ready"));
                this.send({ type: "list" });
                this.send({ type: "listConsumers"});
              }

              if (this._producerSession && msg.roles.includes("producer")) {
                this._producerSession.onProducerRegistered();
              }

              break;
            }

            const peer = normalizePeer(msg, this._channelId);
            if (!peer) {
              break;
            }

            const oldRoles = this._peers[msg.peerId] || [];
            this._peers[msg.peerId] = msg.roles;
            for (const role of ["producer", "consumer"]) {
              if (!oldRoles.includes(role) && msg.roles.includes(role)) {
                this.dispatchEvent(new CustomEvent("peerAdded", { detail: { peer, role } }));
              } else if (oldRoles.includes(role) && !msg.roles.includes(role)) {
                this.dispatchEvent(new CustomEvent("peerRemoved", { detail: { peerId: msg.peerId, role } }));
              }
            }
            break;
          }

          case SignallingServerMessageType.list: {
            this.clearPeers("producer");
            this.addPeers(msg.producers, "producer");
            break;
          }

          case SignallingServerMessageType.listConsumers: {
            this.clearPeers("consumer");
            this.addPeers(msg.consumers, "consumer");
            break;
          }

          case SignallingServerMessageType.sessionStarted:
            {
              const session = this.getConsumerSession(msg.peerId);
              if (session) {
                delete this._consumerSessions[msg.peerId];

                session.onSessionStarted(msg.peerId, msg.sessionId);
                if (session.sessionId && !(session.sessionId in this._consumerSessions)) {
                  this._consumerSessions[session.sessionId] = session;
                } else {
                  session.close();
                }
              }
            }
            break;

          case SignallingServerMessageType.peer:
            {
              const session = this.getConsumerSession(msg.sessionId);
              if (session) {
                session.onSessionPeerMessage(msg);
              } else if (this._producerSession) {
                this._producerSession.onSessionPeerMessage(msg);
              }
            }
            break;

          case SignallingServerMessageType.startSession:
            if (this._producerSession) {
              this._producerSession.onStartSessionMessage(msg);
            }
            break;

          case SignallingServerMessageType.endSession:
            {
              const session = this.getConsumerSession(msg.sessionId);
              if (session) {
                session.close();
              } else if (this._producerSession) {
                this._producerSession.onEndSessionMessage(msg);
              }
            }
            break;

          case SignallingServerMessageType.error:
            this.dispatchEvent(new ErrorEvent("error", {
              message: "error received from signaling server",
              error: new Error(msg.details)
            }));
            break;

          default:
            throw new Error(`unknown message type: "${msg.type}"`);
          }
        }
      } catch (ex) {
        this.dispatchEvent(new ErrorEvent("error", {
          message: "cannot parse incoming message from signaling server",
          error: ex
        }));
      }
    };
  }

  get meta() {
    return this._meta;
  }

  get webrtcConfig() {
    return this._webrtcConfig;
  }

  get ready() {
    return this._ready;
  }

  get channelId() {
    return this._channelId;
  }

  get producerSession() {
    return this._producerSession;
  }

  createProducerSession(stream, consumerId) {
    if (!this._ready || !(stream instanceof MediaStream)) {
      return null;
    }

    if (this._producerSession) {
      if (this._producerSession.stream === stream) {
        return this._producerSession;
      } else {
        return null;
      }
    }

    const session = new ProducerSession(this, stream, consumerId);
    this._producerSession = session;

    session.addEventListener("closed", () => {
      if (this._producerSession === session) {
        this._producerSession = null;
      }
    });

    return session;
  }

  createConsumerSession(producerId, offerOptions) {
    if (!this._ready || !producerId || (typeof (producerId) !== "string")) {
      return null;
    }

    if (offerOptions && (typeof(offerOptions) !== "object")) {
      offerOptions = undefined;
    }

    if (producerId in this._consumerSessions) {
      return this._consumerSessions[producerId];
    }

    for (const session of Object.values(this._consumerSessions)) {
      if (session.peerId === producerId) {
        return session;
      }
    }

    const session = new ConsumerSession(producerId, this, offerOptions);
    this._consumerSessions[producerId] = session;

    session.addEventListener("closed", (event) => {
      let sessionId = event.target.sessionId;
      if (!sessionId) {
        sessionId = event.target.peerId;
      }

      if ((sessionId in this._consumerSessions) && (this._consumerSessions[sessionId] === session)) {
        delete this._consumerSessions[sessionId];
      }
    });

    return session;
  }

  getConsumerSession(sessionId) {
    if (sessionId in this._consumerSessions) {
      return this._consumerSessions[sessionId];
    } else {
      return null;
    }
  }

  closeAllConsumerSessions() {
    for (const session of Object.values(this._consumerSessions)) {
      session.close();
    }

    this._consumerSessions = {};
  }

  send(data) {
    if (this._ready && data && (typeof (data) === "object")) {
      try {
        this._ws.send(JSON.stringify(data));
        return true;
      } catch (ex) {
        this.dispatchEvent(new ErrorEvent("error", {
          message: "cannot send message to signaling server",
          error: ex
        }));
      }
    }

    return false;
  }

  close() {
    if (this._ws) {
      this._ready = false;
      this._channelId = "";
      this._ws.close();

      this.closeAllConsumerSessions();

      if (this._producerSession) {
        this._producerSession.close();
        this._producerSession = null;
      }
    }
  }

  clearPeers(role) {
    for (const peerId in this._peers) {
      if (this._peers[peerId].includes(role)) {
        delete this._peers[peerId];
        this.dispatchEvent(new CustomEvent("peerRemoved", { detail: { peerId, role } }));
      }
    }
  }

  addPeers(items, role) {
    items.forEach(item => {
      const peer = normalizePeer(item, this._channelId);
      if (peer) {
        this._peers[peer.id] = [role];
        this.dispatchEvent(new CustomEvent("peerAdded", { detail: { peer, role } }));
      }
    });
  };
}

export default ComChannel;
