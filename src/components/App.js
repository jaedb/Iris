
import React, { PropTypes } from 'react';
import ReactDOM from 'react-dom';
import { createStore, bindActionCreators } from 'redux'
import { Link } from 'react-router'
import { connect } from 'react-redux'

import * as actions from '../actions/index'
import Services from '../services/Services'


/**
 * The application 'brain'
 *
 * All data handling and fetching is done through this handler
 **/
class App extends React.Component{

	constructor(props){
		super(props);

		this.handleClick = this.handleClick.bind(this);
	}

	handleClick(){
		console.log('click');
		//this.props.providerActions.authorizeSpotify();
		Services.get('services.mopidy')
			.then( function(MopidyService){
				MopidyService.connection.playback.getState()
					.then( function(state){
						console.log('App.js > playback state', state);
					});
			})
	}

	componentDidMount(){
		Services.get('services.mopidy')
			.then( function(MopidyService){
				MopidyService.connection.playback.getState()
					.then( function(state){
						console.log('App.js > playback state', state);
					});
			})
	}

	render(){

		return (
			<div>
	        	<ul role="nav">
					<li><Link to="/queue">Now playing</Link></li>
					<li><Link to="/album/6N51k5TP5pSZYPf7bLffLe">808's and Heartbreak</Link></li>
					<li><Link to="/album/1PgfRdl3lPyACfUGH4pquG">A million</Link></li>
					<li><Link to="/library/albums">Library: Albums</Link></li>
					<li><Link to="/album">Album</Link></li>
					<li><a onClick={this.handleClick}>Authorize</a></li>
		        </ul>
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
		actions: bindActionCreators(actions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(App)