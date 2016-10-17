
export default function reducer(keyboard = {}, action){
	console.log(action);
    switch (action.type) {

        case 'SHIFT_KEY':
            return Object.assign({}, keyboard, { shift: action.down });

        case 'CTRL_KEY':
            return Object.assign({}, keyboard, { ctrl: action.down });

        default:
            return keyboard
    }
}



