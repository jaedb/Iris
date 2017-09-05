let menuItems = [
  {name: "Discover", icon: 'ais', href: "library/browse/ais:root:1"},
  {name: "Playlists", icon: 'playlist', href: "library/playlists"},
  {name: "My Library", icon: 'ais-library', href: "'library/browse/ais:root:2'"},
  {name: "My Family", icon: 'ais-family', href: "library/browse/ais:root:3"},
  {name: "My Likes", icon: 'ais-likes', href: "library/browse/ais:root:4"},
  {name: "Pendrive", icon: 'ais-pendrive', href: "library/browse/ais:root:5"},

];


export default function(state={}, action) {
  switch(action.type) {
    case 'ENABLE_AIS':
      return Object.assign({}, state, {connected: true, menuItems: menuItems})
    default:
      return state
  }
}