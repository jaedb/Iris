
import * as actions from '../actions'

export default function todos(todos = [], action){
    switch( action.type ){

        case actions.LOAD:
            if( localStorage.getItem( 'todos' ) ){
                return JSON.parse( localStorage.getItem( 'todos' ) );
            }
            return [];

        case actions.SAVE:
            localStorage.setItem( 'todos', JSON.stringify(todos) );
            return todos;

        case actions.ADD_TODO:
            return Object.assign([], todos, [...todos, { id: action.id, title: action.title, completed: false }] )

        case actions.TOGGLE_DONE:
            return Object.assign([], todos,
                todos.map(todo => {
                    if( todo.id === action.id ){
                        return Object.assign({}, todo, {
                            completed: !todo.completed
                        })
                    }
                    return todo
                })
            );

        default:
            return todos
    }
}



