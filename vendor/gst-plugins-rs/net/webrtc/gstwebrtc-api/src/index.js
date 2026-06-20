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

import "webrtc-adapter";
import GstWebRTCAPI from "./gstwebrtc-api.js";

/**
 * @external MediaStream
 * @see https://developer.mozilla.org/en-US/docs/Web/API/MediaStream
 */
/**
 * @external RTCPeerConnection
 * @see https://developer.mozilla.org/en-US/docs/Web/API/RTCPeerConnection
 */
/**
 * @external RTCDataChannel
 * @see https://developer.mozilla.org/en-US/docs/Web/API/RTCDataChannel
 */
/**
 * @external RTCOfferOptions
 * @see https://developer.mozilla.org/en-US/docs/Web/API/RTCPeerConnection/createOffer#options
 */
/**
 * @external EventTarget
 * @see https://developer.mozilla.org/en-US/docs/Web/API/EventTarget
 */
/**
 * @external Event
 * @see https://developer.mozilla.org/en-US/docs/Web/API/Event
 */
/**
 * @external ErrorEvent
 * @see https://developer.mozilla.org/en-US/docs/Web/API/ErrorEvent
 */
/**
 * @external CustomEvent
 * @see https://developer.mozilla.org/en-US/docs/Web/API/CustomEvent
 */
/**
 * @external Error
 * @see https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Error
 */
/**
 * @external HTMLVideoElement
 * @see https://developer.mozilla.org/en-US/docs/Web/API/HTMLVideoElement
 */

if (!window.GstWebRTCAPI) {
  window.GstWebRTCAPI = GstWebRTCAPI;
}

export default GstWebRTCAPI;
