
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import TrackList from './TrackList'
import * as spotifyActions from '../services/spotify/actions'

class Artist extends React.Component{

	constructor(props) {
		super(props);
	}

	// on render
	componentDidMount(){
		this.props.spotifyActions.loadArtist( this.props.params.uri );
	}

	// when props changed
	componentWillReceiveProps( nextProps ){
		if( nextProps.params.uri != this.props.params.uri ){
			this.props.spotifyActions.loadArtist( nextProps.params.uri );
		}
	}

	render(){
		if( this.props.spotify.artist ){
			return (
				<div>
					<h4>{ this.props.spotify.artist.name }</h4>
				</div>
			);
		}
		return null;
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
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Artist)