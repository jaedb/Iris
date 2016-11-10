
import React, { PropTypes } from 'react';
import ReactDOM from 'react-dom';
import { createStore, bindActionCreators } from 'redux'
import { Link } from 'react-router'
import { connect } from 'react-redux'

import Sidebar from '../components/Sidebar'
import ContextMenu from '../components/ContextMenu'
import Dragger from '../components/Dragger'

import * as uiActions from '../services/ui/actions'
import * as pusherActions from '../services/pusher/actions'
import * as mopidyActions from '../services/mopidy/actions'
import * as spotifyActions from '../services/spotify/actions'


/**
 * Root level application
 **/
class App extends React.Component{

	constructor(props){
		super(props);
		this.handleKeyUp = this.handleKeyUp.bind(this);
		this.handleKeyDown = this.handleKeyDown.bind(this);
	}

	componentWillMount(){
		window.addEventListener("keyup", this.handleKeyUp, false);
		window.addEventListener("keydown", this.handleKeyDown, false);
	}

	componentDidMount(){
		this.props.pusherActions.connect();
		this.props.mopidyActions.connect();
		this.props.spotifyActions.connect();
		this.props.spotifyActions.getAllLibraryPlaylists();
	}

	componentWillReceiveProps(nextProps){

		// mopidy comes online
		if( !this.props.mopidy_connected && nextProps.mopidy_connected ){
			this.props.mopidyActions.getPlaylists();
		}

		// spotify authorized
		if( !this.props.spotify_authorized && nextProps.spotify_authorized ){
			this.props.spotifyActions.getAllLibraryPlaylists();
		}

		// spotify un-authorized
		if( this.props.spotify_authorized && !nextProps.spotify_authorized ){
			// TODO: flush out playlists and then re-fetch mopidy
		}
	}

	componentWillUnmount(){
		window.removeEventListener("keyup", this.handleKeyUp, false);
		window.removeEventListener("keydown", this.handleKeyDown, false);
	}

	shouldTriggerShortcut(e){
		var ignoreNodes = ['INPUT', 'TEXTAREA'];
		var keyCodes = [32];

		if( ignoreNodes.indexOf(e.target.nodeName) > -1 ){
			return false;
		}

		if( keyCodes.indexOf(e.keyCode) > -1 ){
			e.preventDefault();
			return true;
		}
	}

	handleKeyDown(e){
		this.shouldTriggerShortcut(e);
	}

	handleKeyUp(e){
		if( !this.shouldTriggerShortcut(e) ) return;

		switch(e.keyCode){			
			case 32: // spacebar
				if( this.props.play_state == 'playing' ){
					this.props.mopidyActions.pause();
				}else{
					this.props.mopidyActions.play();
				}
				break;
		}
	}

	render(){
		var className = '';
		if( this.props.dragger && this.props.dragger.dragging ) className += ' dragging'

		return (
			<div className={className}>
		        <Sidebar />
		        <main>
		      		{this.props.children}
		        </main>
		        <ContextMenu state={this.props.context_menu} />
		        <Dragger />
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
	return {
		mopidy_connected: state.mopidy.connected,
		spotify_authorized: state.spotify.authorized,
		play_state: state.mopidy.play_state,
		dragger: state.ui.dragger,
		context_menu: state.ui.context_menu
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		pusherActions: bindActionCreators(pusherActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(App)