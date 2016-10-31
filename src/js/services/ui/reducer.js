
export default function reducer(ui = {}, action){
    switch (action.type) {

        case 'UI_SHOW_CONTEXT_MENU':
            return Object.assign({}, ui, { 
            	context_menu: { 
            		show: true,
            		position_x: action.position_x,
            		position_y: action.position_y,
            		context: action.context, 
            		data: action.data
            	}
            });

        case 'UI_HIDE_CONTEXT_MENU':
            return Object.assign({}, ui, { context_menu: { show: false } });

        case 'UI_LAZY_LOADING':
            return Object.assign({}, ui, { lazy_loading: action.start });

        default:
            return ui
    }
}



