
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'
let helpers = require('../helpers.js')

import TrackList from '../components/TrackList'
import Thumbnail from '../components/Thumbnail'
import Parallax from '../components/Parallax'
import ArtistSentence from '../components/ArtistSentence'
import ArtistGrid from '../components/ArtistGrid'
import Dater from '../components/Dater'
import LazyLoadListener from '../components/LazyLoadListener'

import * as mopidyActions from '../services/mopidy/actions'
import * as spotifyActions from '../services/spotify/actions'

class Album extends React.Component{

	constructor(props) {
		super(props);
	}

	componentDidMount(){
		this.loadAlbum();
	}

	componentWillReceiveProps( nextProps ){
		console.log('componentWillReceiveProps')

		// if our URI has changed, fetch new album
		if( nextProps.params.uri != this.props.params.uri ){
			this.loadAlbum( nextProps )

		// if mopidy has just connected AND we're a local album, go get
		}else if( !this.props.mopidy_connected && nextProps.mopidy_connected ){
			if( helpers.uriSource( this.props.params.uri ) == 'local' ){
				this.loadAlbum( nextProps )
			}
		}
	}

	loadAlbum( props = this.props ){
		switch( helpers.uriSource( props.params.uri ) ){

			case 'spotify':
				this.props.spotifyActions.getAlbum( props.params.uri );
				break;

			case 'local':
				if( props.mopidy_connected ) this.props.mopidyActions.getAlbum( props.params.uri );
				break;
		}
	}

	loadMore(){
		if( !this.props.album.tracks.next ) return
		this.props.spotifyActions.getURL( this.props.album.tracks.next, 'SPOTIFY_ALBUM_LOADED_MORE' );
	}

	renderThumbnail(){
		if( !this.props.album.images ) return <Thumbnail size="large" images={[]} />
		return <Thumbnail size="large" images={ this.props.album.images } />
	}

	render(){
		if( !this.props.album ) return null

		return (
			<div className="view album-view">
				<div className="intro">
					{ this.renderThumbnail() }
					<ArtistGrid artists={ this.props.album.artists } />
					<ul className="details">
						<li>{ this.props.album.tracks.total } tracks, <Dater type="total-time" data={this.props.album.tracks.items} /></li>
						{ this.props.album.release_date ? <li>Released <Dater type="date" data={ this.props.album.release_date } /></li> : null }
						<li><FontAwesome name={helpers.sourceIcon( this.props.params.uri )} /> {helpers.uriSource( this.props.params.uri )} playlist</li>	
					</ul>
				</div>
				<div className="main">

					<div className="title">
						<h1>{ this.props.album.name }</h1>
						<h3><ArtistSentence artists={ this.props.album.artists } /></h3>
					</div>

					<section className="list-wrapper">
						<TrackList tracks={ this.props.album.tracks.items } />
						<LazyLoadListener loadMore={ () => this.loadMore() }/>
					</section>
					
				</div>
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
		album: state.ui.album,
		mopidy_connected: state.mopidy.connected
	};
}

const mapDispatchToProps = (dispatch) => {
	return {
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Album)