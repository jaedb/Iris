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

import getKeysymString from "./keysyms.js";

/** @import ConsumerSession from "./consumer-session.js"; */

const eventsNames = Object.freeze([
  "wheel",
  "contextmenu",
  "mousemove",
  "mousedown",
  "mouseup",
  "touchstart",
  "touchend",
  "touchmove",
  "touchcancel",
  "keyup",
  "keydown"
]);

const mouseEventsNames = Object.freeze({
  mousemove: "MouseMove",
  mousedown: "MouseButtonPress",
  mouseup: "MouseButtonRelease"
});

const touchEventsNames = Object.freeze({
  touchstart: "TouchDown",
  touchend: "TouchUp",
  touchmove: "TouchMotion",
  touchcancel: "TouchUp"
});

const keyboardEventsNames = Object.freeze({
  keydown: "KeyPress",
  keyup: "KeyRelease"
});

function getModifiers(event) {
  const modifiers = [];
  if (event.altKey) {
    modifiers.push("mod1-mask");
  }

  if (event.ctrlKey) {
    modifiers.push("control-mask");
  }

  if (event.metaKey) {
    modifiers.push("meta-mask");
  }

  if (event.shiftKey) {
    modifiers.push("shift-mask");
  }

  return modifiers.join("+");
}

/**
 * Event name: "info".<br>
 * Triggered when a remote peer sends an information message over the control data channel.
 * @event GstWebRTCAPI#InfoEvent
 * @type {CustomEvent}
 * @property {object} detail - The info message
 * @see RemoteController
 */
/**
 * Event name: "controlResponse".<br>
 * Triggered when a remote peer sends a response after a control request.
 * @event GstWebRTCAPI#ControlResponseEvent
 * @type {CustomEvent}
 * @property {object} detail - The response message
 * @see RemoteController
 */

/**
 * @class RemoteController
 * @hideconstructor
 * @classdesc Manages a specific WebRTC data channel created by a remote GStreamer webrtcsink producer and offering
 * remote control of the producer through
 * [GstNavigation]{@link https://gstreamer.freedesktop.org/documentation/video/gstnavigation.html} events.
 * <p>The remote control data channel is created by the GStreamer webrtcsink element on the producer side. Then it is
 * announced through the consumer session thanks to the {@link gstWebRTCAPI#event:RemoteControllerChangedEvent}
 * event.</p>
 * <p>You can attach an {@link HTMLVideoElement} to the remote controller, then all mouse and keyboard events
 * emitted by this element will be automatically relayed to the remote producer.</p>
 * @extends {EventTarget}
 * @fires {@link GstWebRTCAPI#event:ErrorEvent}
 * @fires {@link GstWebRTCAPI#event:ClosedEvent}
 * @fires {@link GstWebRTCAPI#event:InfoEvent}
 * @fires {@link GstWebRTCAPI#event:ControlResponseEvent}
 * @see ConsumerSession#remoteController
 * @see RemoteController#attachVideoElement
 * @see https://gitlab.freedesktop.org/gstreamer/gst-plugins-rs/-/tree/main/net/webrtc/gstwebrtc-api#produce-a-gstreamer-interactive-webrtc-stream-with-remote-control
 */
class RemoteController extends EventTarget {
  constructor(rtcDataChannel, consumerSession) {
    super();

    this._rtcDataChannel = rtcDataChannel;
    this._consumerSession = consumerSession;

    this._videoElement = null;
    this._videoElementComputedStyle = null;
    this._videoElementKeyboard = null;
    this._lastTouchEventTimestamp = 0;
    this._requestCounter = 0;

    rtcDataChannel.addEventListener("close", () => {
      if (this._rtcDataChannel === rtcDataChannel) {
        this.close();
      }
    });

    rtcDataChannel.addEventListener("error", (event) => {
      if (this._rtcDataChannel === rtcDataChannel) {
        const error = event.error;
        this.dispatchEvent(new ErrorEvent("error", {
          message: (error && error.message) || "Remote controller error",
          error: error || new Error("unknown error on the remote controller data channel")
        }));
      }
    });

    rtcDataChannel.addEventListener("message", (event) => {
      try {
        const msg = JSON.parse(event.data);

        if (msg.type === "ControlResponseMessage") {
          this.dispatchEvent(new CustomEvent("controlResponse", { detail: msg }));
        } else if (msg.type === "InfoMessage") {
          this.dispatchEvent(new CustomEvent("info", { detail: msg }));
        }
      } catch (ex) {
        this.dispatchEvent(new ErrorEvent("error", {
          message: "cannot parse control message from signaling server",
          error: ex
        }));
      }
    });
  }


  /**
   * The underlying WebRTC data channel connected to a remote GStreamer webrtcsink producer offering remote control.
   * The value may be null if the remote controller has been closed.
   * @type {RTCDataChannel}
   * @readonly
   */
  get rtcDataChannel() {
    return this._rtcDataChannel;
  }

  /**
   * The consumer session associated with this remote controller.
   * @type {ConsumerSession}
   * @readonly
   */
  get consumerSession() {
    return this._consumerSession;
  }

  /**
   * The video element that is currently used to send all mouse and keyboard events to the remote producer. Value may
   * be null if no video element is attached.
   * @type {HTMLVideoElement}
   * @readonly
   * @see RemoteController#attachVideoElement
   */
  get videoElement() {
    return this._videoElement;
  }

  /**
   * Associates a video element with this remote controller.<br>
   * When a video element is attached to this remote controller, all mouse and keyboard events emitted by this
   * element will be sent to the remote GStreamer webrtcink producer.
   * @param {HTMLVideoElement|null} element - the video element to use to relay mouse and keyboard events,
   * or null to detach any previously attached element. If the provided element parameter is not null and not a
   * valid instance of an {@link HTMLVideoElement}, then the method does nothing.
   */
  attachVideoElement(element) {
    if ((element instanceof HTMLVideoElement) && (element !== this._videoElement)) {
      if (this._videoElement) {
        this.attachVideoElement(null);
      }

      this._videoElement = element;
      this._videoElementComputedStyle = window.getComputedStyle(element);

      for (const eventName of eventsNames) {
        element.addEventListener(eventName, this);
      }

      element.setAttribute("tabindex", "0");
    } else if ((element === null) && this._videoElement) {
      const previousElement = this._videoElement;
      previousElement.removeAttribute("tabindex");

      this._videoElement = null;
      this._videoElementComputedStyle = null;

      this._lastTouchEventTimestamp = 0;

      for (const eventName of eventsNames) {
        previousElement.removeEventListener(eventName, this);
      }
    }
  }

  /**
   * Send a request over the control data channel.<br>
   *
   * @fires {@link GstWebRTCAPI#event:ErrorEvent}
   * @param {object|string} request - The request to send over the channel
   * @returns {number} The identifier attributed to the request, or -1 if an exception occurred
   */
  sendControlRequest(request) {
    try {
      if (!request || ((typeof (request) !== "object") && (typeof (request) !== "string"))) {
        throw new Error("invalid request");
      }

      if (!this._rtcDataChannel) {
        throw new Error("remote controller data channel is closed");
      }

      let message = {
        id: this._requestCounter++,
        request: request
      };

      this._rtcDataChannel.send(JSON.stringify(message));

      return message.id;
    } catch (ex) {
      this.dispatchEvent(new ErrorEvent("error", {
        message: `cannot send control message over session ${this._consumerSession.sessionId} remote controller`,
        error: ex
      }));
      return -1;
    }
  }

  /**
   * Closes the remote controller channel.<br>
   * It immediately shuts down the underlying WebRTC data channel connected to a remote GStreamer webrtcsink
   * producer and detaches any video element that may be used to relay mouse and keyboard events.
   */
  close() {
    this.attachVideoElement(null);

    const rtcDataChannel = this._rtcDataChannel;
    this._rtcDataChannel = null;

    if (rtcDataChannel) {
      rtcDataChannel.close();
      this.dispatchEvent(new Event("closed"));
    }
  }

  _sendGstNavigationEvent(data) {
    let request = {
      type: "navigationEvent",
      event: data
    };
    this.sendControlRequest(request);
  }

  _computeVideoMousePosition(event) {
    const mousePos = { x: 0, y: 0 };
    if (!this._videoElement || (this._videoElement.videoWidth <= 0) || (this._videoElement.videoHeight <= 0)) {
      return mousePos;
    }

    const padding = {
      left: parseFloat(this._videoElementComputedStyle.paddingLeft),
      right: parseFloat(this._videoElementComputedStyle.paddingRight),
      top: parseFloat(this._videoElementComputedStyle.paddingTop),
      bottom: parseFloat(this._videoElementComputedStyle.paddingBottom)
    };

    if (("offsetX" in event) && ("offsetY" in event)) {
      mousePos.x = event.offsetX - padding.left;
      mousePos.y = event.offsetY - padding.top;
    } else {
      const clientRect = this._videoElement.getBoundingClientRect();
      const border = {
        left: parseFloat(this._videoElementComputedStyle.borderLeftWidth),
        top: parseFloat(this._videoElementComputedStyle.borderTopWidth)
      };
      mousePos.x = event.clientX - clientRect.left - border.left - padding.left;
      mousePos.y = event.clientY - clientRect.top - border.top - padding.top;
    }

    const videoOffset = {
      x: this._videoElement.clientWidth - (padding.left + padding.right),
      y: this._videoElement.clientHeight - (padding.top + padding.bottom)
    };

    const ratio = Math.min(videoOffset.x / this._videoElement.videoWidth, videoOffset.y / this._videoElement.videoHeight);
    videoOffset.x = Math.max(0.5 * (videoOffset.x - this._videoElement.videoWidth * ratio), 0);
    videoOffset.y = Math.max(0.5 * (videoOffset.y - this._videoElement.videoHeight * ratio), 0);

    const invRatio = (ratio !== 0) ? (1 / ratio) : 0;
    mousePos.x = (mousePos.x - videoOffset.x) * invRatio;
    mousePos.y = (mousePos.y - videoOffset.y) * invRatio;

    mousePos.x = Math.min(Math.max(mousePos.x, 0), this._videoElement.videoWidth);
    mousePos.y = Math.min(Math.max(mousePos.y, 0), this._videoElement.videoHeight);

    return mousePos;
  }

  handleEvent(event) {
    if (!this._videoElement) {
      return;
    }

    switch (event.type) {
    case "wheel":
      event.preventDefault();
      {
        const mousePos = this._computeVideoMousePosition(event);
        this._sendGstNavigationEvent({
          event: "MouseScroll",
          x: mousePos.x,
          y: mousePos.y,
          delta_x: -event.deltaX, // eslint-disable-line camelcase
          delta_y: -event.deltaY, // eslint-disable-line camelcase
          modifier_state: getModifiers(event) // eslint-disable-line camelcase
        });
      }
      break;

    case "contextmenu":
      event.preventDefault();
      break;

    case "mousemove":
    case "mousedown":
    case "mouseup":
      event.preventDefault();
      {
        const mousePos = this._computeVideoMousePosition(event);
        const data = {
          event: mouseEventsNames[event.type],
          x: mousePos.x,
          y: mousePos.y,
          modifier_state: getModifiers(event) // eslint-disable-line camelcase
        };

        if (event.type !== "mousemove") {
          data.button = event.button + 1;

          if ((event.type === "mousedown") && (event.button === 0)) {
            this._videoElement.focus();
          }
        }

        this._sendGstNavigationEvent(data);
      }
      break;

    case "touchstart":
    case "touchend":
    case "touchmove":
    case "touchcancel":
      for (const touch of event.changedTouches) {
        const mousePos = this._computeVideoMousePosition(touch);
        const data = {
          event: touchEventsNames[event.type],
          identifier: touch.identifier,
          x: mousePos.x,
          y: mousePos.y,
          modifier_state: getModifiers(event) // eslint-disable-line camelcase
        };

        if (("force" in touch) && ((event.type === "touchstart") || (event.type === "touchmove"))) {
          data.pressure = touch.force;
        }

        this._sendGstNavigationEvent(data);
      }

      if (event.timeStamp > this._lastTouchEventTimestamp) {
        this._lastTouchEventTimestamp = event.timeStamp;
        this._sendGstNavigationEvent({
          event: "TouchFrame",
          modifier_state: getModifiers(event) // eslint-disable-line camelcase
        });
      }
      break;
    case "keyup":
    case "keydown":
      event.preventDefault();
      {
        const data = {
          event: keyboardEventsNames[event.type],
          key: getKeysymString(event.key, event.code),
          modifier_state: getModifiers(event) // eslint-disable-line camelcase
        };
        this._sendGstNavigationEvent(data);
      }
      break;
    }
  }
}

export default RemoteController;
