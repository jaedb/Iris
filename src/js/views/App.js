
import React, { PropTypes } from 'react';
import ReactDOM from 'react-dom';
import { createStore, bindActionCreators } from 'redux'
import { Link } from 'react-router'
import { connect } from 'react-redux'

import Player from '../components/Player'

import * as mopidyActions from '../services/mopidy/actions'
import * as spotifyActions from '../services/spotify/actions'


/**
 * Root level application
 **/
class App extends React.Component{

	constructor(props){
		super(props);
	}

	componentDidMount(){
		this.props.mopidyActions.connect();
	}

	render(){

		return (
			<div>
	        	<ul role="nav">
					<li><Link to="/queue">Now playing</Link></li>
					<li><Link to="/library/artists">Library: My artists</Link></li>
					<li><Link to="/library/albums">Library: My albums</Link></li>
					<li><Link to="/library/tracks">Library: My tracks</Link></li>
					<li><Link to="/library/playlists">Library: My playlists</Link></li>
					<li><Link to="/settings">Settings</Link></li>
		        </ul>
		        {this.props.children}
		        <Player />
	        </div>
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
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(App)