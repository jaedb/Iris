
import * as helpers from '../../helpers'

export default function reducer(ui = {}, action){
    switch (action.type){

        case 'LAZY_LOADING':
            return Object.assign({}, ui, { lazy_loading: action.start });

        case 'SET_SLIM_MODE':
            return Object.assign({}, ui, { slim_mode: action.slim_mode });

        case 'DEBUG':
            return Object.assign({}, ui, { debug_response: action.response })

        case 'UI_SET':
            return Object.assign({}, ui, action.data);

        case 'SET_WINDOW_TITLE':
            if (action.title){
                return Object.assign({}, ui, { window_title: action.title });
            } else {
                return ui;
            }

        case 'TOGGLE_SIDEBAR':
            var new_state = !ui.sidebar_open
            if (typeof(action.new_state) !== 'undefined' ) new_state = action.new_state
            return Object.assign({}, ui, { sidebar_open : new_state })

        case 'SET_SELECTED_TRACKS':
            return Object.assign({}, ui, { selected_tracks : Object.assign([],action.keys) })

        case 'ICON_LOADED':
            var icons = Object.assign({}, ui.icons);
            icons[action.key] = action.icon;
            return Object.assign({}, ui, {icons : icons});

        case 'INSTALL_PROMPT':
            return Object.assign({}, ui, {install_prompt : action.event});


        /**
         * Context menu
         **/

        case 'SHOW_CONTEXT_MENU':
            return Object.assign({}, ui, { 
                context_menu: action.data
            });

        case 'HIDE_CONTEXT_MENU':
            return Object.assign(
                {}, 
                ui, 
                {
                    context_menu: Object.assign({}, ui.context_menu, {closing: true})
                }
            );

        case 'REMOVE_CONTEXT_MENU':
            return Object.assign({}, ui, {context_menu: null});

        case 'SHOW_TOUCH_CONTEXT_MENU':
            return Object.assign({}, ui, { 
            	touch_context_menu: action.data
            });

        case 'HIDE_TOUCH_CONTEXT_MENU':
            return Object.assign(
                {}, 
                ui, 
                {
                    touch_context_menu: Object.assign({}, ui.touch_context_menu, {closing: true})
                }
            );

        case 'REMOVE_TOUCH_CONTEXT_MENU':
            return Object.assign({}, ui, {touch_context_menu: null});



        /**
         * Dragging
         **/

        case 'DRAG_START':
            return Object.assign({}, ui, { 
                dragger: { 
                    dragging: true,
                    active: false,
                    context: action.context, 
                    from_uri: action.from_uri, 
                    victims: action.victims,
                    victims_indexes: action.victims_indexes,
                    start_x: action.start_x,
                    start_y: action.start_y
                }
            });

        case 'DRAG_ACTIVE':
            var dragger = Object.assign({}, ui.dragger, { active: true })
            return Object.assign({}, ui, { dragger: dragger });

        case 'DRAG_END':
            return Object.assign({}, ui, { 
                dragger: false
            });




        /**
         * Modals
         **/

        case 'OPEN_MODAL':
            return Object.assign({}, ui, { modal: action.modal })

        case 'CLOSE_MODAL':
            return Object.assign({}, ui, { modal: false })


        /**
         * Notifications
         **/

        case 'CREATE_NOTIFICATION':
            var notifications = Object.assign({}, ui.notifications);
            notifications[action.notification.key] = action.notification;
            return Object.assign({}, ui, { notifications: notifications });

        case 'CLOSE_NOTIFICATION':
            var notifications = Object.assign({}, ui.notifications);
            if (notifications[action.key]){
                notifications[action.key].closing = true;
            }
            return Object.assign({}, ui, { notifications: notifications });

        case 'REMOVE_NOTIFICATION':
            var notifications = Object.assign({}, ui.notifications);
            delete notifications[action.key];
            return Object.assign({}, ui, {notifications: notifications});




        /**
         * Loading and processes
         **/

         case 'START_LOADING':
            var load_queue = Object.assign({}, (ui.load_queue ? ui.load_queue : {}))
            load_queue[action.key] = action.source
            return Object.assign({}, ui, {load_queue: load_queue})

         case 'STOP_LOADING':
            var load_queue = Object.assign({}, (ui.load_queue ? ui.load_queue : {}))
            if (load_queue[action.key]){
                delete load_queue[action.key]
            }
            return Object.assign({}, ui, {load_queue: load_queue})

         case 'START_PROCESS':
         case 'UPDATE_PROCESS':
            var processes = Object.assign({}, (ui.processes ? ui.processes : []))
            if (processes[action.key]){
                var data = Object.assign({}, processes[action.key].data, action.data)
            } else {
                var data = action.data
            }
            processes[action.key] = {
                key: action.key,
                message: action.message,
                status: 'running',
                data: data
            }
            return Object.assign({}, ui, {processes: processes})

         case 'RESUME_PROCESS':
            var processes = Object.assign({}, (ui.processes ? ui.processes : {}))
            if (processes[action.key]){
                processes[action.key] = Object.assign({}, processes[action.key], {status: 'running'})
            }
            return Object.assign({}, ui, {processes: processes})

         case 'CANCEL_PROCESS':
            var processes = Object.assign({}, (ui.processes ? ui.processes : {}))
            if (processes[action.key]){
                processes[action.key] = Object.assign({}, processes[action.key], {status: 'cancelling'})
            }
            return Object.assign({}, ui, {processes: processes})

         case 'PROCESS_CANCELLED':
            var processes = Object.assign({}, (ui.processes ? ui.processes : {}))
            if (processes[action.key]){
                processes[action.key] = Object.assign({}, processes[action.key], {status: 'cancelled'})
            }
            return Object.assign({}, ui, {processes: processes})

         case 'PROCESS_FINISHING':
            var processes = Object.assign({}, (ui.processes ? ui.processes : {}))
            if (processes[action.key]){
                processes[action.key] = Object.assign({}, processes[action.key], {closing: true})
            }
            return Object.assign({}, ui, {processes: processes})

         case 'PROCESS_FINISHED':
            var processes = Object.assign({}, (ui.processes ? ui.processes : {}))
            if (processes[action.key]){
                processes[action.key] = Object.assign({}, processes[action.key], {status: 'finished', closing: false})
            }
            return Object.assign({}, ui, {processes: processes})


        default:
            return ui
    }
}



