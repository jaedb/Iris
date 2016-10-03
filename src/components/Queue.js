
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import Services from '../services/Services'
import TrackList from '../components/TrackList'
import * as actions from '../actions/mopidy'

class Queue extends React.Component{

	constructor(props) {
		super(props);
	}

	renderTracks(){
		if( this.props.mopidy && this.props.mopidy.tracks ){
			return (
				<TrackList tracks={this.props.mopidy.tracks} />
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

export default connect(mapStateToProps, mapDispatchToProps)(Queue)