
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import Tracklist from '../common/Tracklist'
import * as actions from './actions'

class Album extends React.Component{

	constructor(props) {
		super(props);

		this.state = {
			album: false
		}
	}

	// on render
	componentDidMount(){
		this.loadAlbum( this.props.params.id );
	}

	// when props changed
	componentWillReceiveProps( nextProps ){
		if( nextProps.params.id != this.props.params.id ){
			this.loadAlbum( nextProps.params.id );
		}
	}

	loadAlbum( id ){
		let self = this;

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
		if( this.state.album ){
			return (
				<div>
					<h3>{ this.state.album.name }</h3>
					<h3>{ this.state.album.label }</h3>
					<Tracklist tracks={this.state.album.tracks.items} />
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
		actions: bindActionCreators(actions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Album)