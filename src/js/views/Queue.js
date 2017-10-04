
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { hashHistory, Link } from 'react-router'
import { bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'

import Parallax from '../components/Parallax'
import TrackList from '../components/TrackList'
import Track from '../components/Track'
import Dater from '../components/Dater'
import SidebarToggleButton from '../components/SidebarToggleButton'
import FullPlayer from '../components/FullPlayer'
import ArtistSentence from '../components/ArtistSentence'
import Thumbnail from '../components/Thumbnail'
import Header from '../components/Header'

import * as helpers from '../helpers'
import * as uiActions from '../services/ui/actions'
import * as pusherActions from '../services/pusher/actions'
import * as spotifyActions from '../services/spotify/actions'
import * as mopidyActions from '../services/mopidy/actions'

class Queue extends React.Component{

	constructor(props) {
		super(props)
	}

	removeTracks(track_indexes){
		var tlids = []
		for (var i = 0; i < track_indexes.length; i++){
			tlids.push( this.props.current_tracklist[track_indexes[i]].tlid )
		}
		this.props.mopidyActions.removeTracks( tlids )
	}

	playTrack( track ){
		this.props.mopidyActions.changeTrack( track.tlid )
	}

	playTracks( tracks ){
		this.props.mopidyActions.changeTrack( tracks[0].tlid )
	}

	reorderTracks( indexes, index ){
		this.props.mopidyActions.reorderTracklist( indexes, index )
	}

	renderQueueStats(){
		var total_time = 0

		return (
			<div className="queue-stats grey-text">
				<span>{this.props.current_tracklist.length} tracks</span>
				&nbsp;&nbsp;|&nbsp;&nbsp;
				{this.props.current_tracklist.length > 0 ? <Dater type="total-time" data={this.props.current_tracklist} /> : <span>0 mins</span>}
			</div>
		)
	}

	renderArtwork(image){
		if (!image){
			return (
				<span className={this.props.radio_enabled ? 'artwork radio-enabled' : 'artwork'}>
					{this.props.radio_enabled ? <img className="radio-overlay" src="assets/radio-overlay.png" /> : null}
					<Thumbnail circle={this.props.radio_enabled} />
				</span>
			)
		}

		var link = null
		if( this.props.current_track.album.uri ) link = '/album/'+this.props.current_track.album.uri
		return (
			<Link className={this.props.radio_enabled ? 'artwork radio-enabled' : 'artwork'} to={link}>
				{this.props.radio_enabled ? <img className="radio-overlay" src="assets/radio-overlay.png" /> : null}
				<Thumbnail image={image} circle={this.props.radio_enabled} />
			</Link>
		)
	}

	render(){
		var image = null
		if (this.props.current_track && this.props.current_track.images !== undefined){
			image = helpers.sizedImages(this.props.current_track.images)
			image = image.large
		}

		var options = (
			<span>
				<button className="no-hover" onClick={e => this.props.uiActions.openModal('edit_radio')}>
					<FontAwesome name="podcast" />&nbsp;
					Radio
					{this.props.radio && this.props.radio.enabled ? <span className="flag blue">On</span> : null}
				</button>
				<button className="no-hover" onClick={e => hashHistory.push(global.baseURL+'queue/history')}>
					<FontAwesome name="history" />&nbsp;
					History
				</button>
				<button className="no-hover" onClick={e => this.props.mopidyActions.clearTracklist()}>
					<FontAwesome name="trash" />&nbsp;
					Clear
				</button>
				<button className="no-hover" onClick={e => this.props.uiActions.openModal('add_to_queue', {})}>
					<FontAwesome name="plus" />&nbsp;
					Add URI
				</button>
			</span>
		)

		return (
			<div className="view queue-view">			
				<Header icon="play" className="overlay" title="Now playing" options={options} uiActions={this.props.uiActions} />

				<Parallax blur image={image} />

				<div className="content-wrapper">
				
					<div className="current-track">
						{ this.renderArtwork(image) }
						<div className="title">
							{this.props.current_track ? this.props.current_track.name : <span>-</span>}
							&nbsp; <FontAwesome name="info-circle" onClick={e => this.props.uiActions.openModal('track_info')} />
						</div>
						{this.props.current_track ? <ArtistSentence artists={ this.props.current_track.artists } /> : <ArtistSentence />}
					</div>

					<section className="list-wrapper">
						<TrackList
							show_source_icon={true}
							context="queue"
							className="queue-track-list"
							tracks={this.props.current_tracklist}
							removeTracks={tracks => this.removeTracks( tracks )}
							playTracks={tracks => this.playTracks( tracks )}
							playTrack={track => this.playTrack( track )}
							reorderTracks={(indexes, index) => this.reorderTracks(indexes, index) } />
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
		radio: state.core.radio,
		radio_enabled: (state.core.radio && state.core.radio.enabled ? true : false),
		current_tracklist: state.core.current_tracklist,
		current_track: (state.core.current_track !== undefined && state.core.tracks !== undefined && state.core.tracks[state.core.current_track.uri] !== undefined ? state.core.tracks[state.core.current_track.uri] : null)
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		pusherActions: bindActionCreators(pusherActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Queue)