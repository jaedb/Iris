
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
import * as mopidyActions from '../services/mopidy/actions'
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
		if( nextState.sub_view != this.state.sub_view && nextState.sub_view == 'biography' ){
			if( this.props.artist && !this.props.artist.bio ){
				this.props.lastfmActions.getArtist( this.props.artist.name.replace('&','and') )
			}
		}
	}

	loadArtist( props = this.props ){
		var source = helpers.uriSource( props.params.uri );
		if( source == 'spotify' ){
			this.props.spotifyActions.getArtist( props.params.uri );
		}else if( source == 'local' && props.mopidy_connected ){
			this.props.mopidyActions.getArtist( props.params.uri );
		}
	}

	loadMore(){
		if( !this.props.artist.albums_more ) return
		this.props.spotifyActions.getURL( this.props.artist.albums_more, 'SPOTIFY_ARTIST_ALBUMS_LOADED_MORE' );
	}

	play(){
		alert('Yet to be implemented')
	}

	renderSubViewMenu(){		
		return (
			<div className="sub-views">
				<span className={'option '+( this.state.sub_view == 'overview' ? 'active' : null)} onClick={() => this.setState({ sub_view: 'overview' })}>
					Overview
				</span>
				<span className={'option '+( this.state.sub_view == 'related_artists' ? 'active' : null)} onClick={() => this.setState({ sub_view: 'related_artists' })}>
					Related artists
				</span>
				<span className={'option '+( this.state.sub_view == 'biography' ? 'active' : null)} onClick={() => this.setState({ sub_view: 'biography' })}>
					Biography
				</span>
			</div>
		)
	}

	renderBody(){

		if( this.state.sub_view == 'related_artists' ){
			return (
				<div className="body related-artists">
					<h4 className="left-padding">Related artists</h4>
					<section className="grid-wrapper no-top-padding">
						{ this.props.artist.related_artists ? <ArtistGrid artists={ this.props.artist.related_artists } /> : null }
					</section>
				</div>
			)
		}else if( this.state.sub_view == 'biography' ){
			return (
				<div className="body biography">
					<h4 className="left-padding">Biography</h4>
					<section className="text-wrapper no-top-padding">
						{ this.props.artist.bio ? <div><p>{this.props.artist.bio.content}</p><br />
						<div className="grey-text">Published: { this.props.artist.bio.published }</div>
						<div className="grey-text">Origin: <a href={ this.props.artist.bio.links.link.href } target="_blank">{ this.props.artist.bio.links.link.href }</a></div></div> : null }
					</section>
				</div>
			)
		}

		// default body
		return (
			<div className="body overview">
				<div className="col w70">
					<h4 className="left-padding">Top tracks</h4>
					{ this.props.artist.tracks ? <TrackList tracks={ this.props.artist.tracks } /> : null }
				</div>

				<div className="col w5"></div>

				<div className="col w25 related-artists">
					<h4>Related artists</h4>
					{ this.props.artist.related_artists ? <ArtistList artists={ this.props.artist.related_artists.slice(0,6) } /> : null }
				</div>

				<div className="cf"></div>

				<h4 className="left-padding">Albums</h4>
				<section className="grid-wrapper no-top-padding">
					{ this.props.artist.albums ? <AlbumGrid albums={ this.props.artist.albums } /> : null }
					<LazyLoadListener loadMore={ () => this.loadMore() }/>
				</section>
			</div>
		)
	}

	render(){
		if( !this.props.artist ) return null
		var scheme = helpers.uriSource( this.props.params.uri );

		var images = []
		if( this.props.artist.images ) images = this.props.artist.images

		return (
			<div className="view artist-view">

				<SidebarToggleButton />

				<div className="intro">

					<Thumbnail size="huge" images={ images } />
					<Parallax images={ images } />

					<div className="heading-wrapper">
						<div className="heading">
							<h1>{ this.props.artist.name }</h1>
							{ this.renderSubViewMenu() }
						</div>
					</div>

					<div className="details-wrapper">

						<div className="actions">
							<button className="large primary" onClick={ e => this.play() }>Start radio</button>
							{ helpers.uriSource(this.props.params.uri) == 'spotify' ? <FollowButton uri={this.props.params.uri} removeText="Unfollow" addText="Follow" /> : null }						
						</div>

						<ul className="details">
							<li>
								{ this.props.artist.followers ? <span>{ this.props.artist.followers.total.toLocaleString() } followers,&nbsp;</span> : null }
								{ this.props.artist.popularity ? <span>{ this.props.artist.popularity }% popularity</span> : null }
								{ scheme == 'local' ? <span>{ this.props.artist.listeners.toLocaleString() } listeners</span> : null }
							</li>
							{ scheme == 'spotify' ? <li><FontAwesome name='spotify' /> Spotify artist</li> : null }
							{ scheme == 'local' ? <li><FontAwesome name='folder' /> Local artist</li> : null }
						</ul>

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
		artist: state.ui.artist,
		spotify_authorized: state.spotify.authorized,
		mopidy_connected: state.mopidy.connected
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		lastfmActions: bindActionCreators(lastfmActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Artist)