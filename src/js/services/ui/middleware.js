
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
                helpers.setWindowTitle(store.getState().core.current_track, action.data)
                next(action)
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
                if( typeof(action.data) ) data = action.data
                if( typeof(data.data) ) data = Object.assign({}, data, data.data)

                // construct our browser notification
                var title = '';
                var options = {
                    body: '',
                    dir: 'auto',
                    lang: 'EN',
                    tag: 'iris'
                };
                if( data.title ) title = data.title;
                if( data.body ) options.body = data.body;
                if( data.icon ) options.icon = data.icon;

                // make it so
                var notification = new notification( title, options );
                break

            case 'CREATE_NOTIFICATION':

                // start a timeout to remove this notification
                if (!action.notification.sticky){
                    var timeout = setTimeout(
                        function(){
                            store.dispatch(uiActions.removeNotification(action.notification.key))
                        },
                        (action.notification.type == 'shortcut' ? 1000 : 3000)
                    )
                }

                next(action)
                break

            case 'REMOVE_NOTIFICATION':
                var notifications = Object.assign([], store.getState().ui.notifications)

                function getByKey( notification ){
                    return notification.key === action.key
                }
                var index = notifications.findIndex(getByKey)

                // Save our index for the reducer to use. Saves us from re-finding by key
                action.index = index

                // If a broadcast, add to suppressed_broadcasts
                if (index > -1 && typeof(notifications[index]) !== 'undefined' && notifications[index].type == 'broadcast'){
                    store.dispatch({
                        type: 'SUPPRESS_BROADCAST',
                        key: notifications[index].key
                    })
                }

                next(action)
                break

            case 'BROADCASTS_LOADED':
                var suppressed_broadcasts = []
                if (typeof(store.getState().ui.suppressed_broadcasts) !== 'undefined'){
                    suppressed_broadcasts = store.getState().ui.suppressed_broadcasts
                }

                for (var i = 0; i < action.broadcasts.length; i++){
                    var broadcast = action.broadcasts[i]

                    if (!suppressed_broadcasts.includes(broadcast.key)){
                        if (broadcast.message){
                            store.dispatch(uiActions.createNotification(
                                broadcast.message,
                                'broadcast',
                                (broadcast.key ? broadcast.key : null),
                                (broadcast.title ? broadcast.title : null),
                                true
                            )) 
                        }
                    }
                }

                next(action)
                break

            case 'START_PROCESS':
                store.dispatch({
                    type: action.key
                })
                next(action)
                break

            case 'CANCEL_PROCESS':
                store.dispatch({
                    type: action.key+'_CANCEL'
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