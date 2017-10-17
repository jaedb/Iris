
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
import FollowButton from '../components/FollowButton'
import Dater from '../components/Dater'
import LazyLoadListener from '../components/LazyLoadListener'
import ContextMenuTrigger from '../components/ContextMenuTrigger'

import * as helpers from '../helpers'
import * as uiActions from '../services/ui/actions'
import * as mopidyActions from '../services/mopidy/actions'
import * as spotifyActions from '../services/spotify/actions'
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

		// if our URI has changed, fetch new album
		if (nextProps.params.uri != this.props.params.uri){
			this.loadTrack(nextProps )

		// if mopidy has just connected AND we're a local album, go get
		} else if (!this.props.mopidy_connected && nextProps.mopidy_connected){
			if (helpers.uriSource(this.props.params.uri ) != 'spotify'){
				this.loadTrack(nextProps )
			}
		}

		// We don't have lyrics, and we have just received our artists
		if (!nextProps.track.lyrics && !this.props.track.artists && nextProps.track.artists){
			this.props.geniusActions.findTrackLyrics(nextProps.track);
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

	loadTrack(props = this.props){
		switch (helpers.uriSource(props.params.uri)){

			case 'spotify':
				if (props.track){
					console.info('Loading track from index')
				} else {
					this.props.spotifyActions.getTrack(props.params.uri );
				}
				break;

			default:
				if (props.mopidy_connected){
					if (props.track){
						console.info('Loading track from index')
					} else {
						this.props.mopidyActions.getTrack(props.params.uri );
					}
				}
				break;
		}
	}

	play(){
		this.props.mopidyActions.playURIs([this.props.params.uri], this.props.params.uri)
	}

	renderLyricsSelector(){
		if (!this.props.track.lyrics_results){
			return null;
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
				<div className="body-loader loading">
					<div className="loader"></div>
				</div>
			);
		} else if (!this.props.track.lyrics){
			return (
				<div className="lyrics">
					<div className="content">
						<em className="grey-text">No lyrics available</em>
					</div>
				</div>
			)
		} else {
			return (
				<div className="lyrics">
					<div className="content" dangerouslySetInnerHTML={{__html: this.props.track.lyrics}}></div>
					<div className="origin grey-text">
						Origin: <a href={this.props.track.lyrics_url} target="_blank">{this.props.track.lyrics_url}</a>
					</div>
				</div>
			)
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
					<Thumbnail size="large" canZoom images={track.album.images} />
				</div>

				<div className="title">

					<h1>{track.name}</h1>
					<h2 className="grey-text">{track.album ? <Link to={global.baseURL+'album/'+track.album.uri}>{track.album.name}</Link> : "Unknown album"} by <ArtistSentence artists={track.artists} /></h2>

					<ul className="details">
						{!this.props.slim_mode ? <li className="has-tooltip"><FontAwesome name={helpers.sourceIcon(this.props.params.uri)} /><span className="tooltip">{helpers.uriSource(this.props.params.uri)} track</span></li> : null}
						{track.date ? <li><Dater type="date" data={track.date} /></li> : null}
						{track.explicit ? <li><span className="flag dark">EXPLICIT</span></li> : null}
						<li>
							{track.disc_number ? <span>Disc {track.disc_number}</span> : null}
							{track.disc_number && track.track_number ? <span>, </span> : null}
							{track.track_number ? <span>Track {track.track_number}</span> : null}
						</li>
						<li>
							{track.duration ? <Dater type="length" data={track.duration} /> : null}
							{track.length ? <Dater type="length" data={track.length} /> : null}
						</li>
					</ul>
				</div>

				<div className="actions">
					<button className="primary" onClick={e => this.play()}>Play</button>
					{this.props.slim_mode ? null : <ContextMenuTrigger onTrigger={e => this.handleContextMenu(e)} />}
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
	return {
		slim_mode: state.ui.slim_mode,
		load_queue: state.ui.load_queue,
		track: (state.core.tracks && state.core.tracks[ownProps.params.uri] !== undefined ? state.core.tracks[ownProps.params.uri] : false),
		tracks: state.core.tracks,
		artists: state.core.artists,
		albums: state.core.albums,
		spotify_library_albums: state.spotify.library_albums,
		local_library_albums: state.mopidy.library_albums,
		spotify_authorized: state.spotify.authorization,
		mopidy_connected: state.mopidy.connected
	};
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		uiActions: bindActionCreators(uiActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch),
		geniusActions: bindActionCreators(geniusActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Track)