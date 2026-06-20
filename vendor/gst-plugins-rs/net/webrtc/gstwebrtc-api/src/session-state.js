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
 * Session states enumeration.<br>
 * Session state always increases from idle to closed and never switches backwards.
 * @enum {number}
 * @readonly
 */
const SessionState = /** @type {const} */ {
  /**
   * (0) Default state when creating a new session, goes to <i>connecting</i> when starting the session.
   */
  idle: 0,
  /**
   * (1) Session is trying to connect to remote peers, goes to <i>streaming</i> in case of
   * success or <i>closed</i> in case of error.
   */
  connecting: 1,
  /**
   * (2) Session is correctly connected to remote peers and currently streaming audio/video, goes
   * to <i>closed</i> when any peer closes the session.
   */
  streaming: 2,
  /**
   * (3) Session is closed and can be garbage collected, state will not change anymore.
   */
  closed: 3
};
Object.freeze(SessionState);

export default SessionState;
