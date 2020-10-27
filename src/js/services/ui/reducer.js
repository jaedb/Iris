import { omit } from 'lodash';

export default function reducer(ui = {}, action) {
  switch (action.type) {
    case 'LAZY_LOADING':
      return { ...ui, lazy_loading: action.start };

    case 'SET_SLIM_MODE':
      return { ...ui, slim_mode: action.slim_mode };

    case 'SET_WINDOW_FOCUS':
      return { ...ui, window_focus: action.window_focus };

    case 'DEBUG':
      return { ...ui, debug_response: action.response };

    case 'UI_SET':
      return { ...ui, ...action.data };

    case 'SET_WINDOW_TITLE':
      if (action.title) {
        return { ...ui, window_title: action.title };
      }
      return ui;

    case 'TOGGLE_SIDEBAR':
      var new_state = !ui.sidebar_open;
      if (typeof (action.new_state) !== 'undefined') new_state = action.new_state;
      return { ...ui, sidebar_open: new_state };

    case 'SET_CURRENT_TRACK_TRANSITION':
      return { ...ui, current_track_transition: action.current_track_transition };

    case 'SET_SELECTED_TRACKS':
      return { ...ui, selected_tracks: Object.assign([], action.keys) };

    case 'ICON_LOADED':
      var icons = { ...ui.icons };
      icons[action.key] = action.icon;
      return { ...ui, icons };

    case 'INSTALL_PROMPT':
      return { ...ui, install_prompt: action.event };

      /**
         * Context menu
         * */
    case 'SHOW_CONTEXT_MENU':
      return { ...ui, context_menu: action.data };

    case 'HIDE_CONTEXT_MENU':
      return { ...ui, context_menu: { ...ui.context_menu, closing: true } };

    case 'REMOVE_CONTEXT_MENU':
      return { ...ui, context_menu: null };

    case 'SHOW_TOUCH_CONTEXT_MENU':
      return { ...ui, touch_context_menu: action.data };

    case 'HIDE_TOUCH_CONTEXT_MENU':
      return { ...ui, touch_context_menu: { ...ui.touch_context_menu, closing: true } };

    case 'REMOVE_TOUCH_CONTEXT_MENU':
      return { ...ui, touch_context_menu: null };

      /**
         * Dragging
         * */
    case 'DRAG_START':
      return {
        ...ui,
        dragger: {
          dragging: true,
          active: false,
          context: action.context,
          from_uri: action.from_uri,
          victims: action.victims,
          victims_indexes: action.victims_indexes,
          start_x: action.start_x,
          start_y: action.start_y,
        },
      };

    case 'DRAG_ACTIVE':
      var dragger = { ...ui.dragger, active: true };
      return { ...ui, dragger };

    case 'DRAG_END':
      return { ...ui, dragger: false };

      /**
         * Modals
         * */
    case 'OPEN_MODAL':
      return { ...ui, modal: action.modal };

    case 'CLOSE_MODAL':
      return { ...ui, modal: false };


      /**
         * Notifications
         * */
    case 'CREATE_NOTIFICATION':
      var notifications = { ...ui.notifications };
      notifications[action.notification.key] = action.notification;
      return { ...ui, notifications };

    case 'CLOSE_NOTIFICATION':
      var notifications = { ...ui.notifications };
      if (notifications[action.key]) {
        notifications[action.key].closing = true;
      }
      return { ...ui, notifications };

    case 'REMOVE_NOTIFICATION':
      var notifications = { ...ui.notifications };
      delete notifications[action.key];
      return { ...ui, notifications };


      /**
         * Loading and processes
         * */

    case 'START_LOADING':
      var load_queue = { ...(ui.load_queue ? ui.load_queue : {}) };
      load_queue[action.key] = action.source || action.key;
      return { ...ui, load_queue };

    case 'STOP_LOADING':
      return {
        ...ui,
        load_queue: omit(ui.load_queue, action.keys),
      };

    case 'START_PROCESS':
    case 'UPDATE_PROCESS': {
      const processes = { ...(ui.processes || []) };
      processes[action.key] = {
        key: action.key,
        ...processes[action.key] || { status: 'running', notification: true },
        ...(action.type === 'START_PROCESS' ? { status: 'running', remaining: 0, total: 0 } : {}),
        ...action.process,
      };
      return { ...ui, processes };
    }

    case 'CANCEL_PROCESS': {
      const processes = { ...(ui.processes ? ui.processes : {}) };
      action.keys.forEach((key) => {
        if (processes[key]) {
          processes[key] = {
            ...processes[key],
            status: 'cancelling',
            total: undefined,
            remaining: undefined,
          };
        }
      });
      return { ...ui, processes };
    }

    case 'PROCESS_CANCELLED': {
      var processes = { ...(ui.processes ? ui.processes : {}) };
      if (processes[action.key]) {
        processes[action.key] = { ...processes[action.key], status: 'cancelled' };
      }
      return { ...ui, processes };
    }

    case 'PROCESS_FINISHED': {
      var processes = { ...(ui.processes ? ui.processes : {}) };
      if (processes[action.key]) {
        processes[action.key] = { ...processes[action.key], status: 'finished' };
      }
      return { ...ui, processes };
    }

    case 'CLOSE_PROCESS': {
      var processes = { ...(ui.processes ? ui.processes : {}) };
      if (processes[action.key]) {
        processes[action.key] = { ...processes[action.key], closing: true };
      }
      return { ...ui, processes };
    }

    case 'REMOVE_PROCESS': {
      var processes = { ...(ui.processes ? ui.processes : {}) };
      if (processes[action.key]) {
        processes[action.key] = { ...processes[action.key], status: 'completed', closing: false };
      }
      return { ...ui, processes };
    }

    case 'SUPPRESS_BROADCAST':
      return { ...ui, suppressed_broadcasts: [...(ui.suppressed_broadcasts || []), action.key] };

    default:
      return ui;
  }
}
