
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

export function setClientVolume(id, volume) {
  return {
    type: 'SNAPCAST_SET_CLIENT_VOLUME',
    id,
    volume,
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

export function eventReceived(message) {
  return {
    type: 'SNAPCAST_EVENT_RECEIVED',
    method: message.method,
    params: message.params,
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
    flush,
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
