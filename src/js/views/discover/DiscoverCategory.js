
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
	}

	componentWillReceiveProps( nextProps ){
		if( nextProps.params.id != this.props.params.id ){
			this.props.spotifyActions.getCategory( nextProps.params.id );
		}
	}

	loadMore(){
		if( !this.props.playlists_next || !this.props.playlists_next ) return
		this.props.spotifyActions.getURL( this.props.playlists_next, 'SPOTIFY_NEW_RELEASES_LOADED_MORE' );
	}

	render(){
		if (!this.props.category) return null

		var playlists = []
		for (var i = 0; i < this.props.category.playlists.length; i++){
			var uri = this.props.category.playlists[i]
			if (this.props.playlists.hasOwnProperty(uri)){
				playlists.push(this.props.playlists[uri])
			}
		}

		return (
			<div className="view discover-categories-view">
				<Header icon="grid" title={this.props.category.name} />
				<section className="grid-wrapper">
					<PlaylistGrid playlists={playlists} />
				</section>
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
	return {
		playlists: state.ui.playlists,
		category: state.spotify.category
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(DiscoverCategory)