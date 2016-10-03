
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import Services from '../services/Services'
import Tracklist from '../common/Tracklist'
import * as actions from '../actions'

class NowPlaying extends React.Component{

	constructor(props) {
		super(props);
		this.state = {
			tracks: []
		}
	}

	// on render
	componentDidMount(){
		var self = this;
		Services.get('services.mopidy')
			.then( function(MopidyService){
				MopidyService.connection.tracklist.getTlTracks()
					.then( function(tracks){
						self.setState({ tracks : tracks });
					});
			})
	}

	renderTracks(){
		if( this.state.tracks ){
			return (
				<Tracklist tracks={this.state.tracks} />
			);
		}
		return null;
	}

	render(){
		return (
			<div>
				<h3>Now playing</h3>
				{ this.renderTracks() }
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

export default connect(mapStateToProps, mapDispatchToProps)(NowPlaying)