
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import TrackList from './TrackList'
import * as spotifyActions from '../services/spotify/actions'

class Album extends React.Component{

	constructor(props) {
		super(props);
	}

	// on render
	componentDidMount(){
		this.props.spotifyActions.loadAlbum( this.props.params.uri );
	}

	// when props changed
	componentWillReceiveProps( nextProps ){
		if( nextProps.params.uri != this.props.params.uri ){
			this.props.spotifyActions.loadAlbum( nextProps.params.uri );
		}
	}

	loadAlbum( uri ){
		let self = this;

		var id = uri.replace('spotify:album:','');

        $.ajax({
			method: 'GET',
			cache: true,
			url: 'https://api.spotify.com/v1/albums/'+id,
			success: function(album){
        		self.setState({ album: album });
        	}
        });
	}

	renderAlbum(){
		if( this.props.spotify.album ){
			return (
				<div>
					<h3>{ this.props.spotify.album.name }</h3>
					<TrackList tracks={this.props.spotify.album.tracks.items} />
				</div>
			);
		}
		return null;
	}

	render(){
		return (
			<div>
				<h3>Single album</h3>
				{ this.renderAlbum() }
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
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Album)