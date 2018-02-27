
import ReactGA from 'react-ga'

var uiActions = require('./actions.js')
var mopidyActions = require('../mopidy/actions.js')
var helpers = require('../../helpers.js')

const UIMiddleware = (function(){

    /**
     * The actual middleware inteceptor
     **/
    return store => next => action => {
        switch(action.type){

            case 'MOPIDY_STATE':

                // Get the current track from our index
                var tracks = store.getState().core.tracks;
                var current_track_uri = store.getState().core.current_track;
                var current_track = null;
                if (tracks[current_track_uri] !== undefined){
                    current_track = tracks[current_track_uri];
                }
                
                helpers.setWindowTitle(current_track, action.data);
                next(action);
                break

            case 'OPEN_MODAL':
                ReactGA.event({ category: 'Modal', action: 'Opened', label: action.modal.name })
                $('body').addClass('modal-open')
                store.dispatch(uiActions.hideContextMenu())
                store.dispatch(uiActions.hideTouchContextMenu())
                next(action)
                break

            case 'CLOSE_MODAL':
                ReactGA.event({ category: 'Modal', action: 'Closed', label: null })
                $('body').removeClass('modal-open')
                next(action)
                break

            case 'BROWSER_NOTIFICATION':

                var notification = window.Notification || window.mozNotification || window.webkitNotification;
                if ('undefined' === typeof notification) return false;
                if ('undefined' !== typeof notification) notification.requestPermission(function(permission){});

                // handle nested data objects
                var data = {}
                if (typeof(action.data) ) data = action.data
                if (typeof(data.data) ) data = Object.assign({}, data, data.data)

                // construct our browser notification
                var title = '';
                var options = {
                    body: '',
                    dir: 'auto',
                    lang: 'EN',
                    tag: 'iris'
                };
                if (data.title ) title = data.title;
                if (data.body ) options.body = data.body;
                if (data.icon ) options.icon = data.icon;

                // make it so
                var notification = new notification(title, options );
                break

            case 'CREATE_NOTIFICATION':

                // start a timeout to close this notification
                if (!action.notification.sticky){
                    var timeout = setTimeout(
                        function(){
                            store.dispatch(uiActions.closeNotification(action.notification.key))
                        },
                        action.notification.duration * 1000
                    )
                }

                next(action);
                break;

            case 'CLOSE_NOTIFICATION':
                var notifications = Object.assign({}, store.getState().ui.notifications);

                // If a broadcast, add to suppressed_broadcasts
                if (notifications[action.key] && notifications[action.key].type == 'broadcast'){
                    store.dispatch({
                        type: 'SUPPRESS_BROADCAST',
                        key: action.key
                    })
                }

                // start a timeout to remove this notification
                // This gives us time to animate out the notification before we remove the data
                var timeout = setTimeout(
                    function(){
                        store.dispatch(uiActions.removeNotification(action.key))
                    },
                    200
                )

                next(action);
                break;

            case 'BROADCASTS_LOADED':
                var suppressed_broadcasts = []
                if (typeof(store.getState().ui.suppressed_broadcasts) !== 'undefined'){
                    suppressed_broadcasts = store.getState().ui.suppressed_broadcasts
                }

                for (var i = 0; i < action.broadcasts.length; i++){
                    var broadcast = action.broadcasts[i]

                    if (!suppressed_broadcasts.includes(broadcast.key)){
                        if (broadcast.message){
                            var data = {
                                key: (broadcast.key ? broadcast.key : null),
                                title: (broadcast.title ? broadcast.title : null),
                                content: broadcast.message,
                                type: 'broadcast',
                                sticky: true
                            }
                            store.dispatch(uiActions.createNotification(data)); 
                        }
                    }
                }

                next(action)
                break

            case 'START_PROCESS':
                store.dispatch({
                    type: action.key,
                    data: action.data
                })
                store.dispatch({
                    type: action.key+'_STARTED'
                })
                next(action)
                break

            case 'RESUME_PROCESS':
                store.dispatch({
                    type: action.key,
                    data: store.getState().ui.processes[action.key].data
                })
                next(action)
                break

            case 'PROCESS_CANCELLED':
                store.dispatch({
                    type: action.key+'_CANCELLED'
                })
                next(action)
                break

            case 'PROCESS_FINISHED':
                store.dispatch({
                    type: action.key+'_FINISHED'
                })
                next(action)
                break

            // This action is irrelevant to us, pass it on to the next middleware
            default:
                return next(action)
        }
    }

})();

export default UIMiddleware