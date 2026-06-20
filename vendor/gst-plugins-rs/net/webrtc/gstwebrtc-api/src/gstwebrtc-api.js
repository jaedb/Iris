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

import defaultConfig from "./config.js";
import ComChannel from "./com-channel.js";
import SessionState from "./session-state.js";

/** @import ConsumerSession from "./consumer-session.js"; */
/** @import ProducerSession from "./producer-session.js"; */
/** @import GstWebRTCConfig from "./config.js"; */
/** @import RTCOfferOptions from "typescript/lib/lib.dom.js"; */

class GstWebRTCAPI {
  /**
   * @class GstWebRTCAPI
   * @classdesc The API entry point that manages a WebRTC.
   * @constructor
   * @param {GstWebRTCConfig} [userConfig] - The user configuration.<br>
   * Only the parameters different from the default ones need to be provided.
   */
  constructor(userConfig) {
    this._channel = null;
    this._producers = {};
    this._consumers = {};
    this._connectionListeners = [];
    this._peerListeners = [];

    const config = Object.assign({}, defaultConfig);
    if (userConfig && (typeof (userConfig) === "object")) {
      Object.assign(config, userConfig);
    }

    if (typeof (config.meta) !== "object") {
      config.meta = null;
    }

    this._config = config;
    this.connectChannel();
  }

  /**
   * @interface ConnectionListener
   */
  /**
   * Callback method called when this client connects to the WebRTC signaling server.
   * The callback implementation should not throw any exception.
   * @method ConnectionListener#connected
   * @abstract
   * @param {string} clientId - The unique identifier of this WebRTC client.<br>This identifier is provided by the
   * signaling server to uniquely identify each connected peer.
   */
  /**
   * Callback method called when this client disconnects from the WebRTC signaling server.
   * The callback implementation should not throw any exception.
   * @method ConnectionListener#disconnected
   * @abstract
   */

  /**
   * Registers a connection listener that will be called each time the WebRTC API connects to or disconnects from the
   * signaling server.
   * @param {ConnectionListener} listener - The connection listener to register.
   * @returns {boolean} true in case of success (or if the listener was already registered), or false if the listener
   * doesn't implement all callback functions and cannot be registered.
   */
  registerConnectionListener(listener) {
    if (!listener || (typeof (listener) !== "object") ||
      (typeof (listener.connected) !== "function") ||
      (typeof (listener.disconnected) !== "function")) {
      return false;
    }

    if (!this._connectionListeners.includes(listener)) {
      this._connectionListeners.push(listener);
    }

    return true;
  }

  /**
   * Unregisters a connection listener.<br>
   * The removed listener will never be called again and can be garbage collected.
   * @param {ConnectionListener} listener - The connection listener to unregister.
   * @returns {boolean} true if the listener is found and unregistered, or false if the listener was not previously
   * registered.
   */
  unregisterConnectionListener(listener) {
    const idx = this._connectionListeners.indexOf(listener);
    if (idx >= 0) {
      this._connectionListeners.splice(idx, 1);
      return true;
    }

    return false;
  }

  /**
   * Unregisters all previously registered connection listeners.
   */
  unregisterAllConnectionListeners() {
    this._connectionListeners = [];
  }

  /**
   * Creates a new producer session.
   * <p>You can only create one producer session at a time.<br>
   * To request streaming from a new stream you will first need to close the previous producer session.</p>
   * <p>You can only request a producer session while you are connected to the signaling server. You can use the
   * {@link ConnectionListener} interface and {@link GstWebRTCAPI#registerConnectionListener} method to
   * listen to the connection state.</p>
   * @param {MediaStream} stream - The audio/video stream to offer as a producer through WebRTC.
   * @returns {ProducerSession} The created producer session or null in case of error. To start streaming,
   * you still need to call {@link ProducerSession#start} after adding on the returned session all the event
   * listeners you may need.
   */
  createProducerSession(stream) {
    if (this._channel) {
      return this._channel.createProducerSession(stream);
    }
    return null;
  }

  createProducerSessionForConsumer(stream, consumerId) {
    if (this._channel) {
      return this._channel.createProducerSession(stream, consumerId);
    }
    return null;
  }

  /**
   * Information about a remote peer registered by the signaling server.
   * @typedef {object} Peer
   * @readonly
   * @property {string} id - The unique peer identifier set by the signaling server (always non-empty).
   * @property {object} meta - Free-form object containing extra information about the peer (always non-null,
   * but may be empty). Its content depends on your application.
   */

  /**
   * Gets the list of all remote WebRTC producers available on the signaling server.
   * <p>The remote producers list is only populated once you've connected to the signaling server. You can use the
   * {@link ConnectionListener} interface and {@link GstWebRTCAPI#registerConnectionListener} method to
   * listen to the connection state.</p>
   * @returns {Peer[]} The list of remote WebRTC producers available.
   */
  getAvailableProducers() {
    return Object.values(this._producers);
  }

  /**
   * Gets the list of all remote WebRTC consumers available on the signaling server.
   * <p>The remote consumer list is only populated once you've connected to the signaling server. You can use the
   * {@link ConnectionListener} interface and {@link GstWebRTCAPI#registerConnectionListener} method to
   * listen to the connection state.</p>
   * @returns {Peer[]} The list of remote WebRTC consumers available.
   */
  getAvailableConsumers() {
    return Object.values(this._consumers);
  }

  /**
   * @interface PeerListener
   */
  /**
   * Callback method called when a remote producer is added on the signaling server.
   * The callback implementation should not throw any exception.
   * @method PeerListener#producerAdded
   * @abstract
   * @param {Peer} producer - The remote producer added on server-side.
   */
  /**
   * Callback method called when a remote producer is removed from the signaling server.
   * The callback implementation should not throw any exception.
   * @method PeerListener#producerRemoved
   * @abstract
   * @param {Peer} producer - The remote producer removed on server-side.
   */
  /**
   * Callback method called when a remote consumer is added on the signaling server.
   * The callback implementation should not throw any exception.
   * @method PeerListener#consumerAdded
   * @abstract
   * @param {Peer} consumer - The remote consumer added on server-side.
   * */
  /**
   * Callback method called when a remote consumer is removed from the signaling server.
   * The callback implementation should not throw any exception.
   * @method PeerListener#consumerRemoved
   * @abstract
   * @param {Peer} consumer - The remote consumer removed on server-side.
   * */

  /**
   * Registers a listener that will be called each time a peer is added or removed on the signaling server.
   * The listener can implement all or only some of the callback methods.
   * @param {PeerListener} listener - The peer listener to register.
   * @returns {boolean} true in case of success (or if the listener was already registered), or false if the listener
   * doesn't implement any methods from PeerListener and cannot be registered.
   */
  registerPeerListener(listener) {
    // refuse if no methods from PeerListener are implemented
    if (!listener || (typeof (listener) !== "object") ||
      ((typeof (listener.producerAdded) !== "function") &&
       (typeof (listener.producerRemoved) !== "function") &&
       (typeof (listener.consumerAdded) !== "function") &&
       (typeof (listener.consumerRemoved) !== "function"))) {
      return false;
    }

    if (!this._peerListeners.includes(listener)) {
      this._peerListeners.push(listener);
    }

    return true;
  }

  /**
   * Unregisters a peer listener.<br>
   * The removed listener will never be called again and can be garbage collected.
   * @param {PeerListener} listener - The peer listener to unregister.
   * @returns {boolean} true if the listener is found and unregistered, or false if the listener was not previously
   * registered.
   */
  unregisterPeerListener(listener) {
    const idx = this._peerListeners.indexOf(listener);
    if (idx >= 0) {
      this._peerListeners.splice(idx, 1);
      return true;
    }

    return false;
  }

  /**
   * Unregisters all previously registered peer listeners.
   */
  unregisterAllPeerListeners() {
    this._peerListeners = [];
  }

  /**
   * Creates a consumer session by connecting the local client to a remote WebRTC producer.
   * <p>You can only create one consumer session per remote producer.</p>
   * <p>You can only request a new consumer session while you are connected to the signaling server. You can use the
   * {@link ConnectionListener} interface and {@link GstWebRTCAPI#registerConnectionListener} method to
   * listen to the connection state.</p>
   * @param {string} producerId - The unique identifier of the remote producer to connect to.
   * @returns {ConsumerSession} The WebRTC session between the selected remote producer and this local
   * consumer, or null in case of error. To start connecting and receiving the remote streams, you still need to call
   * {@link ConsumerSession#connect} after adding on the returned session all the event listeners you may
   * need.
   */
  createConsumerSession(producerId) {
    if (this._channel) {
      return this._channel.createConsumerSession(producerId);
    }
    return null;
  }

  /**
   * Creates a consumer session by connecting the local client to a remote WebRTC producer and creating the offer.
   * <p>See {@link GstWebRTCAPI#createConsumerSession} for more information</p>
   * @param {string} producerId - The unique identifier of the remote producer to connect to.
   * @param {RTCOfferOptions} offerOptions - An object to use when creating the offer.
   * @returns {ConsumerSession} The WebRTC session between the selected remote producer and this local
   * consumer, or null in case of error. To start connecting and receiving the remote streams, you still need to call
   * {@link ConsumerSession#connect} after adding on the returned session all the event listeners you may
   * need.
   */
  createConsumerSessionWithOfferOptions(producerId, offerOptions) {
    if (this._channel) {
      return this._channel.createConsumerSession(producerId, offerOptions);
    }
    return null;
  }

  connectChannel() {
    if (this._channel) {
      const oldChannel = this._channel;
      this._channel = null;
      oldChannel.close();
      for (const key in this._producers) {
        this.triggerProducerRemoved(key);
      }
      for (const key in this._consumers) {
        this.triggerConsumerRemoved(key);
      }
      this._producers = {};
      this._consumers = {};
      this.triggerDisconnected();
    }

    this._channel = new ComChannel(
      this._config.signalingServerUrl,
      this._config.meta,
      this._config.webrtcConfig
    );

    this._channel.addEventListener("error", (event) => {
      if (event.target === this._channel) {
        console.error(event.message, event.error);
      }
    });

    this._channel.addEventListener("closed", (event) => {
      if (event.target !== this._channel) {
        return;
      }
      this._channel = null;
      for (const key in this._producers) {
        this.triggerProducerRemoved(key);
      }
      for (const key in this._consumers) {
        this.triggerConsumerRemoved(key);
      }
      this._producers = {};
      this._consumers = {};
      this.triggerDisconnected();
      if (this._config.reconnectionTimeout > 0) {
        window.setTimeout(() => {
          this.connectChannel();
        }, this._config.reconnectionTimeout);
      }
    });

    this._channel.addEventListener("ready", (event) => {
      if (event.target === this._channel) {
        this.triggerConnected(this._channel.channelId);
      }
    });

    this._channel.addEventListener("peerAdded", (event) => {
      if (event.target !== this._channel) {
        return;
      }

      if (event.detail.role === "producer") {
        this.triggerProducerAdded(event.detail.peer);
      } else {
        this.triggerConsumerAdded(event.detail.peer);
      }
    });

    this._channel.addEventListener("peerRemoved", (event) => {
      if (event.target !== this._channel) {
        return;
      }

      if (event.detail.role === "producer") {
        this.triggerProducerRemoved(event.detail.peerId);
      } else {
        this.triggerConsumerRemoved(event.detail.peerId);
      }
    });
  }

  triggerConnected(clientId) {
    for (const listener of this._connectionListeners) {
      try {
        listener.connected(clientId);
      } catch (ex) {
        console.error("a listener callback should not throw any exception", ex);
      }
    }
  }

  triggerDisconnected() {
    for (const listener of this._connectionListeners) {
      try {
        listener.disconnected();
      } catch (ex) {
        console.error("a listener callback should not throw any exception", ex);
      }
    }
  }

  triggerProducerAdded(producer) {
    if (producer.id in this._producers) {
      return;
    }

    this._producers[producer.id] = producer;
    for (const listener of this._peerListeners) {
      if (!listener.producerAdded) {
        continue;
      }

      try {
        listener.producerAdded(producer);
      } catch (ex) {
        console.error("a listener callback should not throw any exception", ex);
      }
    }
  }

  triggerProducerRemoved(producerId) {
    if (!(producerId in this._producers)) {
      return;
    }

    const producer = this._producers[producerId];
    delete this._producers[producerId];

    for (const listener of this._peerListeners) {
      if (!listener.producerRemoved) {
        continue;
      }

      try {
        listener.producerRemoved(producer);
      } catch (ex) {
        console.error("a listener callback should not throw any exception", ex);
      }
    }
  }

  triggerConsumerAdded(consumer) {
    if (consumer.id in this._consumers) {
      return;
    }

    this._consumers[consumer.id] = consumer;
    for (const listener of this._peerListeners) {
      if (!listener.consumerAdded) {
        continue;
      }

      try {
        listener.consumerAdded(consumer);
      } catch (ex) {
        console.error("a listener callback should not throw any exception", ex);
      }
    }
  }

  triggerConsumerRemoved(consumerId) {
    if (!(consumerId in this._consumers)) {
      return;
    }

    const consumer = this._consumers[consumerId];
    delete this._consumers[consumerId];

    for (const listener of this._peerListeners) {
      if (!listener.consumerRemoved) {
        continue;
      }

      try {
        listener.consumerRemoved(consumer);
      } catch (ex) {
        console.error("a listener callback should not throw any exception", ex);
      }
    }
  }
}

GstWebRTCAPI.SessionState = SessionState;

export default GstWebRTCAPI;
