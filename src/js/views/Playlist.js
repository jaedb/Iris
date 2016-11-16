
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link } from 'react-router'
import FontAwesome from 'react-fontawesome'
import * as helpers from '../helpers'

import TrackList from '../components/TrackList'
import Thumbnail from '../components/Thumbnail'
import Dater from '../components/Dater'
import ConfirmationButton from '../components/ConfirmationButton'
import LazyLoadListener from '../components/LazyLoadListener'

import * as uiActions from '../services/ui/actions'
import * as mopidyActions from '../services/mopidy/actions'
import * as spotifyActions from '../services/spotify/actions'

class Playlist extends React.Component{

	constructor(props) {
		super(props);
	}

	componentDidMount(){
		this.loadPlaylist();
	}

	componentWillReceiveProps( nextProps ){
		if( nextProps.params.uri != this.props.params.uri ){
			this.loadPlaylist( nextProps )
		}else if( !this.props.mopidy_connected && nextProps.mopidy_connected ){
			if( helpers.uriSource( this.props.params.uri ) == 'm3u' ){
				this.loadPlaylist( nextProps )
			}
		}
	}

	loadPlaylist( props = this.props ){
		switch( helpers.uriSource( props.params.uri ) ){

			case 'spotify':
				this.props.spotifyActions.getPlaylist( props.params.uri );
				break

			case 'm3u':
				if( props.mopidy_connected ) this.props.mopidyActions.getPlaylist( props.params.uri );
				break

		}
	}

	loadMore(){
		if( !this.props.playlist.tracks_more ) return
		this.props.spotifyActions.getURL( this.props.playlist.tracks_more, 'SPOTIFY_PLAYLIST_LOADED_MORE' );
	}

	play(){
		this.props.mopidyActions.playTracks([this.props.params.uri])
	}

	follow(){
		this.props.spotifyActions.toggleFollowingPlaylist( this.props.params.uri, 'PUT' )
	}

	// TODO: Once unfollowing occurs, remove playlist from global playlists list
	unfollow(){
		this.props.spotifyActions.toggleFollowingPlaylist( this.props.params.uri, 'DELETE' )
	}

	// TODO: Once deletion occurs, remove playlist from global playlists list
	delete(){
		this.props.mopidyActions.deletePlaylist( this.props.params.uri )
	}

	reorderTracks( indexes, index ){
		this.props.uiActions.reorderPlaylistTracks( this.props.playlist.uri, indexes, index )
	}

	removeTracks( tracks_indexes ){
		this.props.uiActions.removeTracksFromPlaylist( this.props.playlist.uri, tracks_indexes )
	}

	renderEditButtons(){
		switch( helpers.uriSource( this.props.params.uri ) ){

			case 'm3u':
				return (
					<span>
						<button className="large tertiary" onClick={ e => this.props.uiActions.openModal('edit_playlist', { uri: this.props.playlist.uri, name: this.props.playlist.name }) }>Edit</button>
						<ConfirmationButton className="large tertiary" content="Delete" confirmingContent="Are you sure?" onConfirm={ e => this.delete() } />
					</span>
				)

			case 'spotify':
				if( !this.props.spotify_authorized ) return null
				if( this.props.playlist.owner && this.props.playlist.owner.id == this.props.spotify_userid ){
					return (
						<span>
							<button className="large tertiary" onClick={ e => this.props.uiActions.openModal('edit_playlist', { uri: this.props.playlist.uri, name: this.props.playlist.name, is_public: this.props.playlist.public }) }>Edit</button>
							<ConfirmationButton className="large tertiary" content="Delete" confirmingContent="Are you sure?" onConfirm={ e => this.unfollow() } />
						</span>
					)
				}else if( this.props.playlist.following ){
					return <button className="large tertiary" onClick={ e => this.unfollow() }>Unfollow</button>
				}
				return <button className="large tertiary" onClick={ e => this.follow() }>Follow</button>

		}
	}

	render(){
		if( !this.props.playlist || !this.props.playlist.name ) return null;
		var scheme = helpers.uriSource( this.props.params.uri );

		return (
			<div className="view playlist-view">
				<div className="intro">

					<Thumbnail size="large" images={ this.props.playlist.images } />

					<div className="actions">
						<button className="large primary" onClick={ e => this.play() }>Play</button>
						{ this.renderEditButtons() }
					</div>

					<ul className="details">
						<li>{ this.props.playlist.tracks_total } tracks, <Dater type="total-time" data={this.props.playlist.tracks} /></li>
						{ this.props.playlist.last_modified ? <li>Updated <Dater type="ago" data={this.props.playlist.last_modified} /> ago</li> : null }
						{ this.props.playlist.followers ? <li>{this.props.playlist.followers.total.toLocaleString()} followers</li> : null }
						{ scheme == 'spotify' && this.props.playlist.owner ? <li>By <Link to={'/user/'+this.props.playlist.owner.id}>{this.props.playlist.owner.id}</Link> &nbsp;{ !this.props.playlist.public ? <FontAwesome name="lock" /> : null }</li> : null }
						{ scheme == 'spotify' ? <li><FontAwesome name="spotify" /> Spotify playlist</li> : null }
						{ scheme == 'm3u' ? <li><FontAwesome name="folder" /> Local playlist</li> : null }
					</ul>

				</div>
				<div className="main">

					<div className="title">
						<h1>{ this.props.playlist.name }</h1>
						{ this.props.playlist.description ? <h3 className="grey-text" dangerouslySetInnerHTML={{__html: this.props.playlist.description}}></h3> : null }
					</div>

					<section className="list-wrapper">
						{ this.props.playlist.tracks ? <TrackList context="editable-playlist" tracks={ this.props.playlist.tracks } removeTracks={ tracks_indexes => this.removeTracks(tracks_indexes) } reorderTracks={ (indexes, index) => this.reorderTracks(indexes, index) } /> : null }
						<LazyLoadListener loadMore={ () => this.loadMore() }/>
					</section>
					
				</div>
			</div>
		)
	}
}


/**
 * Export our component
 *
 * We also integrate our global store, using connect()
 **/

const mapStateToProps = (state, ownProps) => {
	return {
		playlist: state.ui.playlist,
		mopidy_connected: state.mopidy.connected,
		spotify_authorized: state.spotify.authorized,
		spotify_userid: state.spotify.me.id
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Playlist)