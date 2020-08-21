
export function setPort(port) {
  return {
    type: 'PUSHER_SET_PORT',
    port,
  };
}

export function setUsername(username, force = false) {
  return {
    type: 'PUSHER_SET_USERNAME',
    username: username.replace(/[\W_]+/g, ''),
    force,
  };
}

export function connect() {
  return {
    type: 'PUSHER_CONNECT',
  };
}

export function disconnect() {
  return {
    type: 'PUSHER_DISCONNECT',
  };
}

export function upgrade() {
  return {
    type: 'PUSHER_UPGRADE',
  };
}

export function reload() {
  return {
    type: 'PUSHER_RELOAD',
  };
}

export function restart() {
  return {
    type: 'PUSHER_RESTART',
  };
}

export function localScan() {
  return {
    type: 'PUSHER_LOCAL_SCAN',
  };
}

export function getConnections() {
  return {
    type: 'PUSHER_GET_CONNECTIONS',
  };
}

export function connectionAdded(connection) {
  return {
    type: 'PUSHER_CONNECTION_ADDED',
    connection,
  };
}

export function updateConnection(connection) {
  return {
    type: 'PUSHER_UPDATE_CONNECTION',
    connection,
  };
}

export function connectionChanged(connection) {
  return {
    type: 'PUSHER_CONNECTION_CHANGED',
    connection,
  };
}

export function connectionRemoved(connection) {
  return {
    type: 'PUSHER_CONNECTION_REMOVED',
    connection,
  };
}

export function request(method, params = null, response_callback = null, error_callback = null) {
  return {
    type: 'PUSHER_REQUEST',
    method,
    params,
    response_callback,
    error_callback,
  };
}

export function getConfig() {
  return {
    type: 'PUSHER_GET_CONFIG',
  };
}

export function getVersion() {
  return {
    type: 'PUSHER_GET_VERSION',
  };
}

export function deliverBroadcast(method, params) {
  return {
    type: 'PUSHER_DELIVER_BROADCAST',
    data: {
      method,
      params,
    },
  };
}

export function deliverMessage(recipient, method, params) {
  return {
    type: 'PUSHER_DELIVER_MESSAGE',
    data: {
      recipient,
      method,
      params,
    },
  };
}

export function getRadio() {
  return {
    type: 'PUSHER_GET_RADIO',
  };
}

export function startRadio(uris) {
  return {
    type: 'PUSHER_START_RADIO',
    uris,
  };
}

export function updateRadio(uris) {
  return {
    type: 'PUSHER_UPDATE_RADIO',
    uris,
  };
}

export function stopRadio() {
  return {
    type: 'PUSHER_STOP_RADIO',
  };
}

export function radioStarted(radio) {
  return {
    type: 'PUSHER_RADIO_STARTED',
    radio,
  };
}

export function radioChanged(radio) {
  return {
    type: 'PUSHER_RADIO_CHANGED',
    radio,
  };
}

export function radioStopped() {
  return {
    type: 'PUSHER_RADIO_STOPPED',
  };
}

export function debug(message = null) {
  return {
    type: 'PUSHER_DEBUG',
    message,
  };
}

export function getQueueMetadata() {
  return {
    type: 'PUSHER_GET_QUEUE_METADATA',
  };
}

export function queueMetadataChanged(queue_metadata) {
  return {
    type: 'PUSHER_QUEUE_METADATA_CHANGED',
    queue_metadata,
  };
}

export function addQueueMetadata(tlids = [], from_uri = null) {
  return {
    type: 'PUSHER_ADD_QUEUE_METADATA',
    tlids,
    from_uri,
  };
}


/**
 * Pinned URIs
 **/

export function getPinned() {
  return {
    type: 'PUSHER_GET_PINNED',
  };
}

export function addPinned(item) {
  return {
    type: 'PUSHER_ADD_PINNED',
    item,
  };
}

export function removePinned(uri) {
  return {
    type: 'PUSHER_REMOVE_PINNED',
    uri,
  };
}

export function setPinned(pinned) {
  return {
    type: 'PUSHER_SET_PINNED',
    pinned,
  };
}

export function pinnedUpdated(pinned) {
  return {
    type: 'PUSHER_PINNED_UPDATED',
    pinned,
  };
}


/**
 * Commands (buttons)
 * */

export function getCommands() {
  return {
    type: 'PUSHER_GET_COMMANDS',
  };
}

export function setCommand(command) {
  return {
    type: 'PUSHER_SET_COMMAND',
    command,
  };
}

export function setCommands(commands) {
  return {
    type: 'PUSHER_SET_COMMANDS',
    commands,
  };
}

export function removeCommand(id) {
  return {
    type: 'PUSHER_REMOVE_COMMAND',
    id,
  };
}

export function runCommand(id, notify = false) {
  return {
    type: 'PUSHER_RUN_COMMAND',
    id,
    notify,
  };
}

export function commandsUpdated(commands) {
  return {
    type: 'PUSHER_COMMANDS_UPDATED',
    commands,
  };
}
