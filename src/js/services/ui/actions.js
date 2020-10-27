import { generateGuid } from '../../util/helpers';
import { handleException } from '../core/actions';

export function getBroadcasts() {
  return (dispatch, getState) => {
    const config = {
      method: 'GET',
      timeout: 15000,
    };

    // Fetch the "iris_broadcasts.json" file from Gist (or "_test" for test mode)
    if (getState().ui.test_mode) {
      config.url = 'https://gist.githubusercontent.com/jaedb/cb3a5ee6909632abb2e0fe66d4c8c311/raw';
    } else {
      config.url = 'https://gist.githubusercontent.com/jaedb/b677dccf80daf3ccb2ef12e96e495677/raw';
    }

    $.ajax(config).then(
      (response) => {
        dispatch({
          type: 'BROADCASTS_LOADED',
          broadcasts: JSON.parse(response),
        });
      },
      (xhr, status, error) => {
        dispatch(
          handleException(
            'Could not fetch broadcasts from GitHub',
            {
              config,
              xhr,
              status,
              error,
            },
            null,
            false,
          ),
        );
      },
    );
  };
}

export function setWindowTitle(title = null, play_state = null) {
  return {
    type: 'SET_WINDOW_TITLE',
    title,
    play_state,
  };
}

/**
 * Gives us the ability to load/fade current track when
 * we're pending transition to a new/next track
 *
 * @param transitioning Boolean
 * */
export function setCurrentTrackTransition(current_track_transition) {
  return {
    type: 'SET_CURRENT_TRACK_TRANSITION',
    current_track_transition,
  };
}

export function setSelectedTracks(keys = []) {
  if (typeof (keys) === 'string') {
    keys = [keys];
  }
  return {
    type: 'SET_SELECTED_TRACKS',
    keys,
  };
}

export function showContextMenu(data) {
  // Touchend
  if (data.e.changedTouches) {
    data.position_x = data.e.changedTouches[0].clientX;
    data.position_y = data.e.changedTouches[0].clientY;

    // Touchstart
  } else if (data.e.touches) {
    data.position_x = data.e.touches[0].clientX;
    data.position_y = data.e.touches[0].clientY;

    // Click/mousedown/mouseup/etc
  } else {
    data.position_x = data.e.clientX;
    data.position_y = data.e.clientY;
  }

  return {
    type: 'SHOW_CONTEXT_MENU',
    data,
  };
}

export function setSlimMode(slim_mode) {
  return {
    type: 'SET_SLIM_MODE',
    slim_mode,
  };
}

export function setWindowFocus(window_focus) {
  return {
    type: 'SET_WINDOW_FOCUS',
    window_focus,
  };
}

export function setLanguage(language) {
  return {
    type: 'SET_LANGUAGE',
    language,
  };
}

export function hideContextMenu() {
  return {
    type: 'HIDE_CONTEXT_MENU',
  };
}

export function removeContextMenu() {
  return {
    type: 'REMOVE_CONTEXT_MENU',
  };
}

export function showTouchContextMenu(data) {
  return {
    type: 'SHOW_TOUCH_CONTEXT_MENU',
    data,
  };
}

export function hideTouchContextMenu() {
  return {
    type: 'HIDE_TOUCH_CONTEXT_MENU',
  };
}

export function removeTouchContextMenu() {
  return {
    type: 'REMOVE_TOUCH_CONTEXT_MENU',
  };
}

export function lazyLoading(start) {
  return {
    type: 'LAZY_LOADING',
    start,
  };
}

export function toggleSidebar(new_state = 'toggle') {
  const action = {
    type: 'TOGGLE_SIDEBAR',
  };
  if (new_state != 'toggle') {
    action.new_state = new_state;
  }
  return action;
}

export function dragStart(e, context, from_uri = null, victims, victims_indexes = null) {
  return {
    type: 'DRAG_START',
    context,
    from_uri,
    victims,
    victims_indexes,
    start_x: e.clientX,
    start_y: e.clientY,
  };
}

export function dragActive() {
  return { type: 'DRAG_ACTIVE' };
}

export function dragEnd() {
  return { type: 'DRAG_END' };
}

export function set(data) {
  return {
    type: 'UI_SET',
    data,
  };
}

export function installPrompt(event) {
  return {
    type: 'INSTALL_PROMPT',
    event,
  };
}


/**
 * Notifications
 *
 * Subtle info/tooltip messages
 * */

export function createBrowserNotification(data) {
  return {
    type: 'BROWSER_NOTIFICATION',
    data,
  };
}

export function createNotification(data) {
  // Allow 'message' as an alias of our content
  if (data.message) {
    data.content = data.message;
  }

  // Shortcut notifications are short and sweet
  if (data.type === 'shortcut') {
    data.duration = 0.4;
  }

  return {
    type: 'CREATE_NOTIFICATION',
    notification: {
      key: generateGuid(),
      duration: 5,
      type: 'default',
      title: null,
      content: null,
      description: null,
      sticky: false,
      closing: false,
      render_content: false,
      ...data,
    },
  };
}

export function closeNotification(key) {
  return {
    type: 'CLOSE_NOTIFICATION',
    key,
  };
}

export function removeNotification(key, manual = false) {
  return {
    type: 'REMOVE_NOTIFICATION',
    key,
    manual,
  };
}


/**
 * Loaders
 * */

export function startLoading(key, source) {
  return {
    type: 'START_LOADING',
    key,
    source,
  };
}

export function stopLoading(keys) {
  return {
    type: 'STOP_LOADING',
    keys: Array.isArray(keys) ? keys : [keys],
  };
}

export function startProcess(key, process) {
  return {
    type: 'START_PROCESS',
    key,
    process,
  };
}

export function updateProcess(key, process) {
  return {
    type: 'UPDATE_PROCESS',
    key,
    process,
  };
}

export function runProcess(key, data = {}) {
  return {
    type: key,
    data,
  };
}

export function cancelProcess(keys) {
  return {
    type: 'CANCEL_PROCESS',
    keys: Array.isArray(keys) ? keys : [keys],
  };
}

export function processCancelled(key) {
  return {
    type: 'PROCESS_CANCELLED',
    key,
  };
}

export function processFinished(key, completionNotification) {
  return {
    type: 'PROCESS_FINISHED',
    key,
    completionNotification,
  };
}

export function closeProcess(key) {
  return {
    type: 'CLOSE_PROCESS',
    key,
  };
}

export function removeProcess(key) {
  return {
    type: 'REMOVE_PROCESS',
    key,
  };
}
