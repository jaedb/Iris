
export function set(data) {
  return {
    type: 'SNAPCAST_SET',
    data,
  };
}

export function setConnection(data) {
  return {
    type: 'SNAPCAST_SET_CONNECTION',
    data,
  };
}

export function setEnabled(enabled) {
  return {
    type: 'SNAPCAST_SET_ENABLED',
    enabled,
  };
}

export function connect() {
  return {
    type: 'SNAPCAST_CONNECT',
  };
}

export function disconnect() {
  return {
    type: 'SNAPCAST_DISCONNECT',
  };
}

export function request(method, params = null, response_callback = null, error_callback = null) {
  return {
    type: 'SNAPCAST_REQUEST',
    method,
    params,
    response_callback,
    error_callback,
  };
}

export function debug(message = null) {
  return {
    type: 'SNAPCAST_DEBUG',
    message,
  };
}

export function getServer() {
  return {
    type: 'SNAPCAST_GET_SERVER',
  };
}

export function setClientName(id, name) {
  return {
    type: 'SNAPCAST_SET_CLIENT_NAME',
    id,
    name,
  };
}

export function setClientMute(id, mute) {
  return {
    type: 'SNAPCAST_SET_CLIENT_MUTE',
    id,
    mute,
  };
}

export function setClientVolume(id, volume, group_id = null) {
  return {
    type: 'SNAPCAST_SET_CLIENT_VOLUME',
    id,
    volume,
    group_id,
  };
}

export function setClientLatency(id, latency) {
  return {
    type: 'SNAPCAST_SET_CLIENT_LATENCY',
    id,
    latency,
  };
}

export function setClientGroup(id, group_id) {
  return {
    type: 'SNAPCAST_SET_CLIENT_GROUP',
    id,
    group_id,
  };
}

export function deleteClient(id) {
  return {
    type: 'SNAPCAST_DELETE_CLIENT',
    id,
  };
}

export function setGroupName(id, name) {
  return {
    type: 'SNAPCAST_SET_GROUP_NAME',
    id,
    name,
  };
}

export function setGroupStream(id, stream_id) {
  return {
    type: 'SNAPCAST_SET_GROUP_STREAM',
    id,
    stream_id,
  };
}

export function setGroupMute(id, mute) {
  return {
    type: 'SNAPCAST_SET_GROUP_MUTE',
    id,
    mute,
  };
}

export function setGroupVolume(id, percent, old_percent = 0) {
  return {
    type: 'SNAPCAST_SET_GROUP_VOLUME',
    id,
    percent,
    old_percent,
  };
}

export function calculateGroupVolume(id, clients) {
  return {
    type: 'SNAPCAST_CALCULATE_GROUP_VOLUME',
    id,
    clients,
  };
}



/**
 * Record loaders
 *
 * We've got a loaded record, now we just need to plug it in to our state and stores.
 * */

export function serverLoaded(server) {
  return {
    type: 'SNAPCAST_SERVER_LOADED',
    server,
  };
}

export function clientLoaded(client) {
  return clientsLoaded([client]);
}
export function clientsLoaded(clients, flush = false) {
  return {
    type: 'SNAPCAST_CLIENTS_LOADED',
    clients,
    flush,
  };
}

export function groupLoaded(group) {
  return groupsLoaded([group]);
}
export function groupsLoaded(groups, flush = false) {
  return {
    type: 'SNAPCAST_GROUPS_LOADED',
    groups,
  };
}

export function streamLoaded(stream) {
  return streamsLoaded([stream]);
}
export function streamsLoaded(streams, flush = false) {
  return {
    type: 'SNAPCAST_STREAMS_LOADED',
    streams,
    flush,
  };
}
