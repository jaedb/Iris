
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import Header from '../../components/Header'
import PlaylistGrid from '../../components/PlaylistGrid'
import LazyLoadListener from '../../components/LazyLoadListener'

import * as spotifyActions from '../../services/spotify/actions'

class DiscoverCategory extends React.Component{

	constructor(props) {
		super(props);
	}

	componentDidMount(){
		this.props.spotifyActions.getCategory( this.props.params.id );
		this.props.spotifyActions.getCategoryPlaylists( this.props.params.id );
	}

	componentWillReceiveProps( nextProps ){
		if( nextProps.params.id != this.props.params.id ){
			this.props.spotifyActions.getCategory( nextProps.params.id );
			this.props.spotifyActions.getCategoryPlaylists( nextProps.params.id );
		}
	}

	loadMore(){
		if( !this.props.spotify.new_releases || !this.props.spotify.new_releases.next ) return
		this.props.spotifyActions.getURL( this.props.spotify.new_releases.next, 'SPOTIFY_NEW_RELEASES_LOADED_MORE' );
	}

	render(){
		if( !this.props.spotify.category ) return null;

		return (
			<div className="view discover-categories-view">
				<Header icon="grid" title={this.props.spotify.category.name} />
				{ this.props.spotify.category_playlists ? <PlaylistGrid playlists={this.props.spotify.category_playlists.items} /> : null }
				<LazyLoadListener loadMore={ () => this.loadMore() }/>
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

export default connect(mapStateToProps, mapDispatchToProps)(DiscoverCategory)