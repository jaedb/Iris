
export default function(state={}, action) {
  switch(action.type) {
    case 'ENABLE_AIS':
      return Object.assign({}, state, {connected: true})
    default:
      return state
  }
}