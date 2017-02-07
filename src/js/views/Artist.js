
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'

import LazyLoadListener from '../components/LazyLoadListener'
import Header from '../components/Header'
import TrackList from '../components/TrackList'
import AlbumGrid from '../components/AlbumGrid'
import Thumbnail from '../components/Thumbnail'
import Parallax from '../components/Parallax'
import ArtistList from '../components/ArtistList'
import ArtistGrid from '../components/ArtistGrid'
import FollowButton from '../components/FollowButton'
import SidebarToggleButton from '../components/SidebarToggleButton'

import * as helpers from '../helpers'
import * as uiActions from '../services/ui/actions'
import * as mopidyActions from '../services/mopidy/actions'
import * as pusherActions from '../services/pusher/actions'
import * as lastfmActions from '../services/lastfm/actions'
import * as spotifyActions from '../services/spotify/actions'

class Artist extends React.Component{

	constructor(props) {
		super(props)

		this.state = {
			sub_view: 'overview'
		}
	}

	componentDidMount(){
		this.loadArtist();
	}

	componentWillReceiveProps( nextProps ){
		if( nextProps.params.uri != this.props.params.uri ){
			this.loadArtist( nextProps )
		}else if( !this.props.mopidy_connected && nextProps.mopidy_connected ){
			if( helpers.uriSource( this.props.params.uri ) == 'local' ){
				this.loadArtist( nextProps )
			}
		}
	}

	componentWillUpdate( nextProps, nextState ){
		if( nextState.sub_view != this.state.sub_view && nextState.sub_view == 'about' ){
			if( this.props.artist && !this.props.artist.bio ){
				this.props.lastfmActions.getArtist( this.props.params.uri, this.props.artist.name.replace('&','and') )
			}
		}
	}

	loadArtist( props = this.props ){
		switch( helpers.uriSource( props.params.uri ) ){

			case 'spotify':
				if (props.artist && props.artist.albums_uris && props.artist.related_artists_uris){
					console.info('Loading spotify artist from index')
				}else{
					this.props.spotifyActions.getArtist( props.params.uri );
				}
				break

			case 'local':
				if (props.mopidy_connected){
					if (props.artist && props.artist.images){
						console.info('Loading local artist from index')
					} else {
						this.props.mopidyActions.getArtist( props.params.uri );
					}
				}
				break
		}
		
		// go back to overview
		this.setState({ sub_view: 'overview' })
	}

	loadMore(){
		this.props.spotifyActions.getURL( this.props.artist.albums_more, 'SPOTIFY_ARTIST_ALBUMS_LOADED', this.props.params.uri );
	}

	renderSubViewMenu(){		
		return (
			<div className="sub-views">
				<span className={'option '+( this.state.sub_view == 'overview' ? 'active' : null)} onClick={() => this.setState({ sub_view: 'overview' })}>
					Overview
				</span>
				{this.props.artist.related_artists_uris ? <span className={'option '+( this.state.sub_view == 'related_artists' ? 'active' : null)} onClick={() => this.setState({ sub_view: 'related_artists' })}>Related artists</span> : null}
				<span className={'option '+( this.state.sub_view == 'about' ? 'active' : null)} onClick={() => this.setState({ sub_view: 'about' })}>
					About
				</span>
			</div>
		)
	}

	renderBody(){
		var scheme = helpers.uriSource( this.props.params.uri );

		var related_artists = []
		if (this.props.artist.related_artists_uris){
			for (var i = 0; i < this.props.artist.related_artists_uris.length; i++){
				var uri = this.props.artist.related_artists_uris[i]
				if (this.props.artists.hasOwnProperty(uri)){
					related_artists.push(this.props.artists[uri])
				}
			}
		}

		var albums = []
		if (this.props.artist.albums_uris){
			for (var i = 0; i < this.props.artist.albums_uris.length; i++){
				var uri = this.props.artist.albums_uris[i]
				if (this.props.albums.hasOwnProperty(uri)){
					albums.push(this.props.albums[uri])
				}
			}
		}

		if( this.state.sub_view == 'related_artists' ){
			return (
				<div className="body related-artists">
					<h4 className="left-padding">Related artists</h4>
					<section className="grid-wrapper no-top-padding">
						<ArtistGrid artists={related_artists} />
					</section>
				</div>
			)
		}else if( this.state.sub_view == 'about' ){
			return (
				<div className="body about">
					<h4 className="left-padding">Biography</h4>

					<ul className="details">
						<li>
							{ this.props.artist.followers ? <span>{ this.props.artist.followers.total.toLocaleString() } followers,&nbsp;</span> : null }
							{ this.props.artist.popularity ? <span>{ this.props.artist.popularity }% popularity</span> : null }
							{ this.props.artist.listeners && scheme == 'local' ? <span>{ this.props.artist.listeners.toLocaleString() } listeners</span> : null }
						</li>
						{ scheme == 'spotify' ? <li><FontAwesome name='spotify' /> Spotify artist</li> : null }
						{ scheme == 'local' ? <li><FontAwesome name='folder' /> Local artist</li> : null }
					</ul>

					<section className="text-wrapper no-top-padding">
						{ this.props.artist.bio ? <div><p>{this.props.artist.bio.content}</p><br />
						<div className="grey-text">Published: { this.props.artist.bio.published }</div>
						<div className="grey-text">Origin: <a href={ this.props.artist.bio.links.link.href } target="_blank">{ this.props.artist.bio.links.link.href }</a></div></div> : null }
					</section>
				</div>
			)
		}

		return (
			<div className="body overview">
				<div className={related_artists.length > 0 ? "col w70" : "col w100"}>
					<h4 className="left-padding">Top tracks</h4>
					{ this.props.artist.tracks ? <TrackList tracks={this.props.artist.tracks} /> : null }
				</div>

				<div className="col w5"></div>

				{related_artists.length > 0 ? <div className="col w25 related-artists"><h4>Related artists</h4><ArtistList artists={related_artists.slice(0,6)} /></div> : null}

				<div className="cf"></div>

				<h4 className="left-padding">Albums</h4>
				<section className="grid-wrapper no-top-padding">
					<AlbumGrid albums={albums} />
					<LazyLoadListener enabled={this.props.artist.albums_more} loadMore={ () => this.loadMore() }/>
				</section>
			</div>
		)
	}

	render(){
		if( !this.props.artist ) return null
		var scheme = helpers.uriSource( this.props.params.uri );

		var image = null
		if( this.props.artist.images ) image = helpers.sizedImages( this.props.artist.images ).huge

		return (
			<div className="view artist-view">

				<SidebarToggleButton />

				<div className="intro">

					<Parallax image={ image } />

					<div className="liner">
						<h1>{ this.props.artist.name }</h1>
						<div className="actions">
							{ scheme == 'spotify' ? <button className="primary rounded" onClick={e => this.props.pusherActions.startRadio([this.props.artist.uri])}>Start radio</button> : null }
							{ scheme == 'spotify' ? <FollowButton className="outline rounded white" uri={this.props.params.uri} removeText="Unfollow" addText="Follow" is_following={this.props.artist.is_following} /> : null }						
						</div>
						{ this.renderSubViewMenu() }
					</div>
				</div>

				{ this.renderBody() }

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
		artist: (state.ui.artists && typeof(state.ui.artists[ownProps.params.uri]) !== 'undefined' ? state.ui.artists[ownProps.params.uri] : false ),
		artists: state.ui.artists,
		albums: state.ui.albums,
		spotify_authorized: state.spotify.authorized,
		mopidy_connected: state.mopidy.connected
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		pusherActions: bindActionCreators(pusherActions, dispatch),
		lastfmActions: bindActionCreators(lastfmActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Artist)