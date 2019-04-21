
export default state => {

    // Changed at 3.36
    if (state.ui.shortkeys_enabled !== undefined){
        state.ui.hotkeys_enabled = state.ui.shortkeys_enabled;
    }
    return state;
}