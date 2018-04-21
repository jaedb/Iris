
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { Link } from 'react-router'
import { bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'

import Header from '../components/Header'
import TrackList from '../components/TrackList'
import Thumbnail from '../components/Thumbnail'
import ArtistSentence from '../components/ArtistSentence'
import ArtistGrid from '../components/ArtistGrid'
import FollowButton from '../components/Fields/FollowButton'
import LastfmLoveButton from '../components/Fields/LastfmLoveButton'
import Dater from '../components/Dater'
import LazyLoadListener from '../components/LazyLoadListener'
import ContextMenuTrigger from '../components/ContextMenuTrigger'
import URILink from '../components/URILink'

import * as helpers from '../helpers'
import * as uiActions from '../services/ui/actions'
import * as mopidyActions from '../services/mopidy/actions'
import * as spotifyActions from '../services/spotify/actions'
import * as lastfmActions from '../services/lastfm/actions'
import * as geniusActions from '../services/genius/actions'

class Track extends React.Component{

	constructor(props){
		super(props);
	}

	componentDidMount(){
		this.loadTrack();
	}

	handleContextMenu(e){
		e.preventDefault()
		var data = { uris: [this.props.params.uri] }
		this.props.uiActions.showContextMenu(e, data, 'track', 'click' )
	}

	componentWillReceiveProps(nextProps){

		// if our URI has changed, fetch new track
		if (nextProps.params.uri != this.props.params.uri){
			this.loadTrack(nextProps)

		// if mopidy has just connected AND we're not a Spotify track, go get
		} else if (!this.props.mopidy_connected && nextProps.mopidy_connected){
			if (helpers.uriSource(this.props.params.uri) != 'spotify'){
				this.loadTrack(nextProps);
			}
		}

		// We have just received our full track info (with artists)
		if (!this.props.track.artists && nextProps.track.artists){

			// Ready to load LastFM
			if (nextProps.lastfm_authorized){
				this.props.lastfmActions.getTrack(nextProps.track.uri);
			}

			// Ready to load lyrics
			if (!nextProps.track.lyrics_results){
				this.props.geniusActions.findTrackLyrics(nextProps.track);
			}
		}
	}

	handleContextMenu(e){
		var data = {
			e: e,
			context: 'track',
			items: [this.props.track],
			uris: [this.props.params.uri]
		}
		this.props.uiActions.showContextMenu(data)
	}

	/**
	 * TODO: Identify why images being loaded breaks the thumbnail. Is there a new image array format
	 * we need to accommodate? 
	 **/
	loadTrack(props = this.props){
		switch (helpers.uriSource(props.params.uri)){

			case 'spotify':
				if (props.track){
					console.info('Loading track from index');
				} else {
					this.props.spotifyActions.getTrack(props.params.uri);
					this.props.spotifyActions.following(props.params.uri);
				}
				break;

			default:
				if (props.mopidy_connected){
					if (props.track){
						console.info('Loading track from index');
					} else {
						this.props.mopidyActions.getTrack(props.params.uri );
					}
				}
				break;
		}

		// We have artist info already
		if (props.track && props.track.artists){

			// Get the LastFM version of this track (provided we have artist info)
			if (props.lastfm_authorized){
				this.props.lastfmActions.getTrack(props.track.uri);
			}

			// Ready for lyrics
			if (props.track && !props.track.lyrics_results){
				this.props.geniusActions.findTrackLyrics(props.track);
			}
		}
	}

	play(){
		this.props.mopidyActions.playURIs([this.props.params.uri], this.props.params.uri)
	}

	renderLyricsSelector(){
		if (this.props.track.lyrics_results === undefined || this.props.track.lyrics_results === null){
			return null;

		} else if (this.props.track.lyrics_results.length <= 0){
			return (
				<div className="field lyrics-selector">
					<div className="input">
						<input type="text" disabled="disabled" value="No results" />
						<div className="description">
							Switch to another lyrics seach result
						</div>
					</div>
				</div>
			);
		}

		return (
			<div className="field lyrics-selector">
				<div className="input">
					<select
						onChange={e => this.props.geniusActions.getTrackLyrics(this.props.track.uri, e.target.value)}>
						{
							this.props.track.lyrics_results.map(result => {
								return (
									<option 
										key={result.url} 
										value={result.url}
										defaultValue={result.url == this.props.track.lyrics_url}
									>
											{result.title}
									</option>
								)
							})
						}
					</select>
					<div className="description">
						Switch to another lyrics seach result
					</div>
				</div>
			</div>
		);
	}

	renderLyrics(){
		if (helpers.isLoading(this.props.load_queue,['genius_'])){
			return (
				<div className="lyrics">
					<div className="body-loader loading">
						<div className="loader"></div>
					</div>
				</div>
			);
		} else if (this.props.track.lyrics){
			return (
				<div className="lyrics">
					<div className="content" dangerouslySetInnerHTML={{__html: this.props.track.lyrics}}></div>
					<div className="origin grey-text">
						Origin: <a href={this.props.track.lyrics_url} target="_blank">{this.props.track.lyrics_url}</a>
					</div>
				</div>
			)
		} else {
			return null;
		}
	}

	render(){
		if (helpers.isLoading(this.props.load_queue,['spotify_track/'+helpers.getFromUri('trackid',this.props.params.uri)])){
			return (
				<div className="body-loader loading">
					<div className="loader"></div>
				</div>
			)
		}

		if (!this.props.track){
			return null
		} else {
			var track = this.props.track
		}

		return (
			<div className="view track-view content-wrapper">

				{this.props.slim_mode ? <Header 
					icon="music" 
					title="Track" 
					handleContextMenuTrigger={e => this.handleContextMenu(e)} 
					uiActions={this.props.uiActions} /> : null}

				<div className="thumbnail-wrapper">
					<URILink type="album" uri={track.album ? track.album.uri : null}>
						<Thumbnail size="large" canZoom images={track.images} />
					</URILink>
				</div>

				<div className="title">

					<h1>{track.name}</h1>
					<h2 className="grey-text">
						{track.album && track.album.uri ? <Link to={global.baseURL+'album/'+track.album.uri}>{track.album.name}</Link> : null}
						{track.album && !track.album.uri ? track.album.name : null}
						{!track.album ? "Unknown album" : null}
						&nbsp;by <ArtistSentence artists={track.artists} />
					</h2>

					<ul className="details">
						{!this.props.slim_mode ? <li className="has-tooltip"><FontAwesome name={helpers.sourceIcon(this.props.params.uri)} /><span className="tooltip">{helpers.uriSource(this.props.params.uri)} track</span></li> : null}
						{track.date ? <li><Dater type="date" data={track.date} /></li> : null}
						{track.explicit ? <li><span className="flag dark">EXPLICIT</span></li> : null}
						<li>
							{track.disc_number ? <span>Disc {track.disc_number}</span> : null}
							{track.disc_number && track.track_number ? <span>, </span> : null}
							{track.track_number ? <span>Track {track.track_number}</span> : null}
						</li>
						{track.duration ? <li><Dater type="length" data={track.duration} /></li> : null}
					</ul>
				</div>

				<div className="actions">
					<button className="primary" onClick={e => this.play()}>Play</button>
					<LastfmLoveButton uri={this.props.params.uri} artist={this.props.track.artists[0].name} track={this.props.track.name} addText="Love" removeText="Unlove" is_loved={this.props.track.userloved} />
					<ContextMenuTrigger onTrigger={e => this.handleContextMenu(e)} />
				</div>

				{this.renderLyricsSelector()}
				{this.renderLyrics()}

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
	var uri = ownProps.params.uri;
	return {
		slim_mode: state.ui.slim_mode,
		load_queue: state.ui.load_queue,
		track: (state.core.tracks && state.core.tracks[uri] !== undefined ? state.core.tracks[uri] : false),
		tracks: state.core.tracks,
		artists: state.core.artists,
		albums: state.core.albums,
		spotify_library_albums: state.spotify.library_albums,
		local_library_albums: state.mopidy.library_albums,
		lastfm_authorized: state.lastfm.session,
		spotify_authorized: state.spotify.authorization,
		mopidy_connected: state.mopidy.connected
	};
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		uiActions: bindActionCreators(uiActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		lastfmActions: bindActionCreators(lastfmActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch),
		geniusActions: bindActionCreators(geniusActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Track)