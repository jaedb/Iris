
export default (state) => {
  if (state.ui.shortkeys_enabled !== undefined) {
    state.ui.hotkeys_enabled = state.ui.shortkeys_enabled;
  }
  return state;
};
