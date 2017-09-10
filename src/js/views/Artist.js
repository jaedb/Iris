
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link } from 'react-router'
import FontAwesome from 'react-fontawesome'

import LazyLoadListener from '../components/LazyLoadListener'
import Header from '../components/Header'
import TrackList from '../components/TrackList'
import AlbumGrid from '../components/AlbumGrid'
import Thumbnail from '../components/Thumbnail'
import Parallax from '../components/Parallax'
import ArtistGrid from '../components/ArtistGrid'
import RelatedArtists from '../components/RelatedArtists'
import FollowButton from '../components/FollowButton'
import SidebarToggleButton from '../components/SidebarToggleButton'
import ContextMenuTrigger from '../components/ContextMenuTrigger'

import * as helpers from '../helpers'
import * as uiActions from '../services/ui/actions'
import * as mopidyActions from '../services/mopidy/actions'
import * as pusherActions from '../services/pusher/actions'
import * as lastfmActions from '../services/lastfm/actions'
import * as spotifyActions from '../services/spotify/actions'

class Artist extends React.Component{

	constructor(props) {
		super(props)
	}

	componentDidMount(){
		this.loadArtist()
	}

	componentWillReceiveProps( nextProps ){
		if( nextProps.params.uri != this.props.params.uri ){
			this.loadArtist( nextProps )
		}else if( !this.props.mopidy_connected && nextProps.mopidy_connected ){
			if( helpers.uriSource( this.props.params.uri ) != 'spotify' ){
				this.loadArtist( nextProps )
			}
		}
	}

	handleContextMenu(e){
		var data = {
			e: e,
			context: 'artist',
			items: [this.props.artist],
			uris: [this.props.params.uri]
		}
		this.props.uiActions.showContextMenu(data)
	}

	loadArtist( props = this.props ){
		switch( helpers.uriSource( props.params.uri ) ){

			case 'spotify':
				if (props.artist && props.artist.albums_uris && props.artist.related_artists_uris){
					console.info('Loading spotify artist from index')
				}else{
					this.props.spotifyActions.getArtist(props.params.uri, true);
				}
				break

			default:
				if (props.mopidy_connected){
					if (props.artist && props.artist.images){
						console.info('Loading local artist from index')
					} else {
						this.props.mopidyActions.getArtist(props.params.uri);
					}
				}
				break
		}
	}

	loadMore(){
		this.props.spotifyActions.getURL( this.props.artist.albums_more, 'SPOTIFY_ARTIST_ALBUMS_LOADED', this.props.params.uri );
	}

	inLibrary(){
		var library = helpers.uriSource(this.props.params.uri)+'_library_artists'
		return (this.props[library] && this.props[library].indexOf(this.props.params.uri) > -1)
	}

	renderSubViewMenu(){		
		return (
			<div className="sub-views">
				<Link className="option" activeClassName="active" to={global.baseURL+'artist/'+this.props.params.uri}><h4>Overview</h4></Link>
				{this.props.artist.related_artists_uris ? <Link className="option" activeClassName="active" to={global.baseURL+'artist/'+this.props.params.uri+'/related-artists'}><h4>Related artists</h4></Link> : null}
				<Link className="option" activeClassName="active" to={global.baseURL+'artist/'+this.props.params.uri+'/about'}><h4>About</h4></Link>
			</div>
		)
	}

	renderBody(){
		if (helpers.isLoading(this.props.load_queue,['spotify_artists/'+helpers.getFromUri('artistid',this.props.params.uri), 'lastfm_method=artist.getInfo'])){
			return (
				<div className="body-loader loading">
					<div className="loader"></div>
				</div>
			)
		}
		
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

		switch (this.props.params.sub_view){

			case 'related-artists':
				return (
					<div className="body related-artists">
						<section className="grid-wrapper no-top-padding">
							<ArtistGrid artists={related_artists} />
						</section>
					</div>
				)

			case 'about':
				return (
					<div className="body about">

						<div className="col w40 tiles artist-stats">
							{this.props.artist.images ? <div className="tile thumbnail-wrapper"><Thumbnail size="huge" canZoom images={this.props.artist.images} /></div> : null}
							{this.props.artist.images_additional ? <div className="tile thumbnail-wrapper"><Thumbnail size="huge" canZoom images={this.props.artist.images_additional} /></div> : null}
							{this.props.artist.followers ? <div className="tile"><span className="content"><FontAwesome name="users" />{this.props.artist.followers.total.toLocaleString() } followers</span></div> : null}
							{this.props.artist.popularity ? <div className="tile"><span className="content"><FontAwesome name="fire" />{this.props.artist.popularity }% popularity</span></div> : null}
							{this.props.artist.listeners ? <div className="tile"><span className="content"><FontAwesome name="headphones" />{ this.props.artist.listeners.toLocaleString() } listeners</span></div> : null }
						</div>

						<div className="col w60 biography">
							<section>
								{ this.props.artist.bio ? <div className="biography-text"><p>{this.props.artist.bio.content}</p><br />
								<div className="grey-text">Published: { this.props.artist.bio.published }</div>
								<div className="grey-text">Origin: <a href={ this.props.artist.bio.links.link.href } target="_blank">{ this.props.artist.bio.links.link.href }</a></div></div> : null }
							</section>
						</div>
					</div>
				)

			default:
				return (
					<div className="body overview">
						<div className={related_artists.length > 0 ? "col w70" : "col w100"}>
							<h4>Top tracks</h4>
							<div className="list-wrapper">
								{ this.props.artist.tracks ? <TrackList className="artist-track-list" uri={this.props.params.uri} tracks={this.props.artist.tracks} /> : null }
							</div>
						</div>

						<div className="col w5"></div>

						{related_artists.length > 0 ? <div className="col w25 related-artists"><h4>Related artists</h4><div className="list-wrapper"><RelatedArtists artists={related_artists.slice(0,6)} /></div><Link to={global.baseURL+'artist/'+this.props.params.uri+'/related-artists'} className="button">All related artists</Link></div> : null}

						<div className="cf"></div>

						<h4>Albums</h4>
						<section className="grid-wrapper no-top-padding">
							<AlbumGrid albums={albums} />
							<LazyLoadListener loading={this.props.artist.albums_more} loadMore={ () => this.loadMore() }/>
						</section>
					</div>
				)
		}
	}

	render(){

		var scheme = helpers.uriSource( this.props.params.uri )

		if ( this.props.artist && this.props.artist.images ){
			var image = helpers.sizedImages( this.props.artist.images ).huge
		} else {
			var image = null
		}

		if (this.props.artist){
			var is_spotify = (scheme == 'spotify')
			if (is_spotify){
				var uris_to_play = (this.props.artist.tracks ? helpers.arrayOf('uri', this.props.artist.tracks) : [])
			} else {
				var uris_to_play = this.props.artist.albums_uris
			}

			return (
				<div className="view artist-view">

					{this.props.slim_mode ? <Header className="overlay" icon="mic" title="Artist" handleContextMenuTrigger={e => this.handleContextMenu(e)} uiActions={this.props.uiActions} /> : null}

					<div className="intro">

						<Parallax image={image} />

						<div className="liner">
							<h1>{this.props.artist ? this.props.artist.name : null}</h1>
							<div className="actions">
								<button className="primary" onClick={e => this.props.mopidyActions.playURIs(uris_to_play, this.props.artist.uri)}>Play</button>
								{is_spotify ? <FollowButton className="white" uri={this.props.params.uri} removeText="Remove from library" addText="Add to library" is_following={this.inLibrary()} /> : null}
								{this.props.slim_mode ? null : <ContextMenuTrigger className="white" onTrigger={e => this.handleContextMenu(e)} />}
							</div>
							{ this.renderSubViewMenu() }
						</div>
					</div>
					<div className="content-wrapper">
						{this.renderBody()}
					</div>
				</div>
			);

		} else {
			return (
				<div className="view artist-view">
					<SidebarToggleButton />
					<div className="intro">
						<Parallax />
						<div className="liner">
							<h1>
								<span className="placeholder"></span>
							</h1>
							<div className="actions">
								<button className="placeholder">&nbsp;</button>
								<button className="placeholder">&nbsp;</button>
							</div>
							{ this.renderSubViewMenu() }
						</div>
					</div>
					<div className="content-wrapper">
						{this.renderBody()}
					</div>
				</div>
			);
		}
	}
}


/**
 * Export our component
 *
 * We also integrate our global store, using connect()
 **/

const mapStateToProps = (state, ownProps) => {
	return {
		slim_mode: state.ui.slim_mode,
		load_queue: state.ui.load_queue,
		artist: (state.core.artists && typeof(state.core.artists[ownProps.params.uri]) !== 'undefined' ? state.core.artists[ownProps.params.uri] : false ),
		artists: (state.core.artists ? state.core.artists : []),
		spotify_library_artists: state.spotify.library_artists,
		local_library_artists: state.mopidy.library_artists,
		albums: (state.core.albums ? state.core.albums : []),
		spotify_authorized: state.spotify.authorization,
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