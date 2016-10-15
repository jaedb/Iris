
import React, { PropTypes } from 'react';
import ReactDOM from 'react-dom';
import { createStore, bindActionCreators } from 'redux'
import { Link } from 'react-router'
import { connect } from 'react-redux'

import * as actions from '../actions/index'
import * as mopidyActions from '../services/mopidy/actions'
import * as spotifyActions from '../services/spotify/actions'
import SpotifyAuthenticationFrame from './SpotifyAuthenticationFrame'


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
					<li><Link to="/library/albums">Library: Albums</Link></li>
		        </ul>
		        <SpotifyAuthenticationFrame />
		        {this.props.children}
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
		actions: bindActionCreators(actions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(App)