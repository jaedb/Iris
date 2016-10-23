
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link } from 'react-router'

import Player from '../components/Player'

import FontAwesome from 'react-fontawesome'
import * as mopidyActions from '../services/mopidy/actions'

class Sidebar extends React.Component{

	constructor(props) {
		super(props);
	}

	componentDidMount(){
		console.log( this.props )
	}

	render(){
		return (
			<aside>

	        	<nav>

	        		<section>
						<Link activeClassName="active" to="/queue">Now playing</Link>
					</section>

					<section>
						<title>Discover</title>
					</section>

					<section>
						<title>My Music</title>
						<Link activeClassName="active" to="/library/playlists">Playlists</Link>
						<Link activeClassName="active" to="/library/artists">Artists</Link>
						<Link activeClassName="active" to="/library/albums">Albums</Link>
						<Link activeClassName="active" to="/library/tracks">Tracks</Link>
					</section>

					<section>
						<Link activeClassName="active" to="/settings">Settings</Link>
					</section>

		        </nav>

		        <Player />

			</aside>
		);
	}
}


/**
 * Export our component
 *
 * We also integrate our global store, using connect()
 **/

const mapStateToProps = (state, ownProps) => {
	return state;
}

const mapDispatchToProps = (dispatch) => {
	return {
		mopidyActions: bindActionCreators(mopidyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Sidebar)