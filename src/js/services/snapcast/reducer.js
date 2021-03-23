export default function reducer(snapcast = {}, action) {
  switch (action.type) {
    case 'SNAPCAST_SET':
      return { ...snapcast, ...action.data };

    case 'SNAPCAST_CONNECTED':
      return { ...snapcast, connected: true, connecting: false };

    case 'SNAPCAST_CONNECTING':
      return { ...snapcast, connecting: true, connected: false };

    case 'SNAPCAST_DISCONNECTED':
      return { ...snapcast, connected: false, connecting: false };

    case 'SNAPCAST_SERVER_LOADED':
      return { ...snapcast, server: action.server };

    case 'SNAPCAST_CLIENTS_LOADED':
      return { ...snapcast, clients: action.clients };

    case 'SNAPCAST_GROUPS_LOADED':
      if (action.flush) {
        var groups = {};
      } else {
        var groups = { ...snapcast.groups };
      }

      for (const group of action.groups) {
        groups[group.id] = {
          ...(groups[group.id] ? groups[group.id] : {}),
          ...group,
        };
      }
      return { ...snapcast, groups };

    case 'SNAPCAST_STREAMS_LOADED':
      if (action.flush) {
        var streams = {};
      } else {
        var streams = { ...snapcast.streams };
      }

      for (const stream of action.streams) {
        streams[stream.id] = stream;
      }
      return { ...snapcast, streams };

    default:
      return snapcast;
  }
}
