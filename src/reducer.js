/**
 * Root reducer
 *
 * This combines all our application reducers into a single, top-level reducer.
 * We need to combine reducers to create the unified application-level store.
 **/

// import all of our application reducers
import todos from './todos/reducer'
import config from './config/reducer'

import { combineReducers } from 'redux'

// combine them into one root reducer
export default combineReducers({
    todos,
    config
})



