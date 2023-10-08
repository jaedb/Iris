export default function reducer(mopidy = {}, action) {
  switch (action.type) {
    case 'MOPIDY_SET':
      return { ...mopidy, ...action.data };

    case 'MOPIDY_UPDATE_SERVERS':
      return { ...mopidy, servers: action.servers };

    case 'MOPIDY_CONNECT':
    case 'MOPIDY_CONNECTING':
      return {
        ...mopidy,
        connected: false,
        connecting: true,
      };

    case 'MOPIDY_CONNECTED':
      return {
        ...mopidy,
        connected: true,
        connecting: false,
        restart_running: false,
        upgrade_running: false,
      };

    case 'MOPIDY_DISCONNECTED':
      return {
        ...mopidy,
        connected: false,
        connecting: false,
      };

    case 'MOPIDY_CHANGE_TRACK':
      return { ...mopidy, tlid: action.tlid };

    case 'MOPIDY_URI_SCHEMES':
      return { ...mopidy, uri_schemes: action.uri_schemes };

    case 'MOPIDY_RESTART_STARTED':
      return { ...mopidy, restart_running: true };

    case 'MOPIDY_RESTART_FINISHED':
      return { ...mopidy, restart_running: false };

    case 'MOPIDY_UPGRADE_STARTED':
      return { ...mopidy, upgrade_running: true };

    case 'MOPIDY_UPGRADE_FINISHED':
      return { ...mopidy, upgrade_running: false };

    case 'MOPIDY_LOCAL_SCAN_STARTED':
      return { ...mopidy, local_scan_running: true };

    case 'MOPIDY_LOCAL_SCAN_FINISHED':
      return { ...mopidy, local_scan_running: false };

    /**
     * State-oriented actions
     * */
    case 'MOPIDY_PLAY_STATE':
      return { ...mopidy, play_state: action.play_state };

    case 'MOPIDY_CONSUME':
      return { ...mopidy, consume: action.consume };

    case 'MOPIDY_RANDOM':
      return { ...mopidy, random: action.random };

    case 'MOPIDY_REPEAT':
      return { ...mopidy, repeat: action.repeat };

    case 'MOPIDY_VOLUME':
      return { ...mopidy, volume: action.volume };

    case 'MOPIDY_MUTE':
      return { ...mopidy, mute: action.mute };

    case 'MOPIDY_TIME_POSITION':
      return { ...mopidy, time_position: action.time_position };

    case 'MOPIDY_QUEUE_HISTORY':
      var history = [];
      for (let i = 0; i < action.tracks.length; i++) {
        history.push({

          ...action.tracks[i][1],
          played_at: action.tracks[i][0],
          type: 'history',
        });
      }
      return { ...mopidy, queue_history: history };

    /**
     * Directories
     * This also facilitates all backend-only music providers (SoundCloud, Dirble, etc)
     * */
    case 'MOPIDY_DIRECTORY_LOADING':
      return { ...mopidy, directory: { uri: action.uri } };

    case 'MOPIDY_DIRECTORY_LOADED':
      return {
        ...mopidy,
        directory: { ...mopidy.directory, ...action.directory },
      };

    default:
      return mopidy;
  }
}
