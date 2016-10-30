
import React, { PropTypes } from 'react';
import ReactDOM from 'react-dom';
import { createStore, bindActionCreators } from 'redux'
import { Link } from 'react-router'
import { connect } from 'react-redux'

import Sidebar from '../components/Sidebar'
import ContextMenu from '../components/ContextMenu'

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
		this.props.pusherActions.connect();
		this.props.mopidyActions.connect();

		window.addEventListener("keyup", this.handleKeyUp, false);
		window.addEventListener("keydown", this.handleKeyDown, false);
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
				if( this.props.mopidy.state == 'playing' ){
					this.props.mopidyActions.pause();
				}else{
					this.props.mopidyActions.play();
				}
				break;
		}
	}

	render(){
		return (
			<div>
		        <Sidebar />
		        <main>
		      		{this.props.children}
		        </main>
		      	<footer>
		      		Iris by James Barnsley &nbsp;|&nbsp; v{ this.props.pusher.version.current }
		      	</footer>
		        { this.props.ui.context_menu.test }
		        <ContextMenu state={this.props.ui.context_menu} />
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
		uiActions: bindActionCreators(uiActions, dispatch),
		pusherActions: bindActionCreators(pusherActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(App)