export default (state) => {
  if (state.ui.shortkeys_enabled !== undefined) {
    state.ui.hotkeys_enabled = state.ui.shortkeys_enabled;
  }

  // Change authorizations from 'null' to 'undefined' to fix destructuring issues like
  // that of https://github.com/jaedb/Iris/issues/662
  if (state.spotify && state.spotify.me === null) state.spotify.me = undefined;
  if (state.lastfm && state.lastfm.me === null) state.lastfm.me = undefined;
  if (state.genius && state.genius.me === null) state.genius.me = undefined;

  return state;
};
