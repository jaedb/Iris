
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import Link from './Link'
import { bindActionCreators } from 'redux'

import ProgressSlider from './Fields/ProgressSlider'
import VolumeControl from './Fields/VolumeControl'
import MuteControl from './Fields/MuteControl'
import OutputControl from './Fields/OutputControl'
import Dater from './Dater'
import ArtistSentence from './ArtistSentence'
import Thumbnail from './Thumbnail'
import Icon from './Icon'

import * as helpers from '../helpers'
import * as uiActions from '../services/ui/actions'
import * as coreActions from '../services/core/actions'
import * as mopidyActions from '../services/mopidy/actions'

class PlaybackControls extends React.Component{

	constructor(props){
		super(props)
		this.stream = null;
		this.state = {
			expanded: false
		}
	}

	componentDidMount(){
		if (this.props.http_streaming_enabled){

			// Bust our cache, and by consequence, play our stream
			this.props.coreActions.cachebustHttpStream();
		}
	}

	playStream(props = this.props){
		
		if (!this.stream){
			this.stream = new Audio();
		} else {
			this.stream.src = null;
		}

		if (!props.http_streaming_enabled || !props.http_streaming_url){
			return false;
		}

		this.stream.src = props.http_streaming_url+"?cb="+props.http_streaming_cachebuster;
		this.stream.muted = props.http_streaming_mute;
		this.stream.volume = props.http_streaming_volume / 100;
		this.stream.play();

		console.log("Playing stream: "+this.stream.src);
	}

	componentWillReceiveProps(nextProps){

		// Cachebuster changed
		// This happens when playback changes, so that the stream is "new", rather
		// than the original stream. This prevents the browser cache from starting
		// the stream right from the beginning (which could be hours of continuous playback).
		if (this.props.http_streaming_cachebuster !== nextProps.http_streaming_cachebuster){
			this.playStream(nextProps);
		}

		// Just been enabled
		if (!this.props.http_streaming_enabled && nextProps.http_streaming_enabled){
			this.playStream(nextProps);
		}

		if (this.stream){

			// Just been muted
			if (this.props.http_streaming_mute !== nextProps.http_streaming_mute){
				this.stream.muted = nextProps.http_streaming_mute;
			}

			// Just had volume changed
			if (this.props.http_streaming_volume !== nextProps.http_streaming_volume){
				this.stream.volume = nextProps.http_streaming_volume / 100;
			}

			// Just been disabled
			if (!nextProps.http_streaming_enabled){
				this.stream = null;
			}
		}
	}

	renderPlayButton(){
		var button = <a className="control play" onClick={() => this.props.mopidyActions.play()}><Icon name="play_circle_filled" type="material" /></a>
		if (this.props.play_state == 'playing'){
			button = <a className="control play" onClick={() => this.props.mopidyActions.pause()}><Icon name="pause_circle_filled" type="material" /></a>
		}
		return button;
	}

	renderConsumeButton(){
		var button = <a className="control tooltip" onClick={() => this.props.mopidyActions.setConsume(true)}><Icon name="restaurant" type="material" /><span className="tooltip__content">Consume</span></a>
		if (this.props.consume){
			button = <a className="control control--active tooltip" onClick={() => this.props.mopidyActions.setConsume(false)}><Icon name="restaurant" type="material" /><span className="tooltip__content">Consume</span></a>
		}
		return button;
	}

	renderRandomButton(){
		var button = <a className="control tooltip" onClick={() => this.props.mopidyActions.setRandom(true)}><Icon name="shuffle" type="material" /><span className="tooltip__content">Shuffle</span></a>
		if (this.props.random){
			button = <a className="control control--active tooltip" onClick={() => this.props.mopidyActions.setRandom(false)}><Icon name="shuffle" type="material" /><span className="tooltip__content">Shuffle</span></a>
		}
		return button;
	}

	renderRepeatButton(){
		var button = <a className="control tooltip" onClick={() => this.props.mopidyActions.setRepeat(true)}><Icon name="repeat" /><span className="tooltip__content">Repeat</span></a>
		if (this.props.repeat){
			button = <a className="control control--active tooltip" onClick={() => this.props.mopidyActions.setRepeat(false)}><Icon name="repeat" /><span className="tooltip__content">Repeat</span></a>
		}
		return button;
	}

	render(){
		var images = false
		if (this.props.current_track && this.props.current_track.images){
			images = this.props.current_track.images
		}

		return (
			<div className={(this.state.expanded ? "playback-controls--expanded playback-controls" : "playback-controls")}>

				{this.props.next_track && this.props.next_track.images ? <Thumbnail className="hide" size="large" images={this.props.next_track.images} /> : null}
				
				<div className="current-track">
					<Link className="thumbnail-wrapper" to={global.baseURL+'kiosk-mode'}>
						<Thumbnail size="small" images={images} />
					</Link>
					<div className="title">
						{ this.props.current_track ? this.props.current_track.name : <span>-</span> }
					</div>
					<div className="artist">
						{ this.props.current_track ? <ArtistSentence artists={ this.props.current_track.artists } /> : <ArtistSentence /> }
					</div>
				</div>

				<section className="playback">
					<a className="control previous" onClick={() => this.props.mopidyActions.previous()}>
						<Icon name="skip_previous" type="material" />
					</a>
					{ this.renderPlayButton() }
					<a className="control stop" onClick={() => this.props.mopidyActions.stop()}>
						<Icon name="stop" type="material" />
					</a>
					<a className="control next" onClick={() => this.props.mopidyActions.next()}>
						<Icon name="skip_next" type="material" />
					</a>
				</section>

				<section className="settings">
					{this.renderConsumeButton()}
					{this.renderRandomButton()}
					{this.renderRepeatButton()}
					<OutputControl force_expanded={this.state.expanded} />
				</section>

				<section className="progress">
					<ProgressSlider />
					<span className="current">{ this.props.time_position ? <Dater type="length" data={this.props.time_position} /> : '-' }</span>
					<span className="total">{ this.props.current_track ? <Dater type="length" data={this.props.current_track.duration} /> : '-' }</span>
				</section>

				<section className="volume">
					<MuteControl 
						mute={this.props.mute}
						onMuteChange={mute => this.props.mopidyActions.setMute(mute)}
					/>
					<VolumeControl 
						scrollWheel
						volume={this.props.volume}
						mute={this.props.mute}
						onVolumeChange={percent => this.props.mopidyActions.setVolume(percent)}
					/>
				</section>

				<section className="triggers">
					<a className="control expanded-controls" onClick={() => this.setState({expanded: !this.state.expanded})}>
						{this.state.expanded ? <Icon name="expand_more" type="material" /> : <Icon name="expand_less" type="material" />}
					</a>
					<a className={"control sidebar-toggle"+(this.props.sidebar_open ? ' open' : '')} onClick={() => this.props.uiActions.toggleSidebar()}>
						<Icon className="open" name="menu" type="material" />
					</a>
				</section>
				
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
		snapcast_enabled: state.pusher.config.snapcast_enabled,
		http_streaming_enabled: state.core.http_streaming_enabled,
		http_streaming_volume: (state.core.http_streaming_volume ? state.core.http_streaming_volume : 50),
		http_streaming_mute: (state.core.http_streaming_mute ? state.core.http_streaming_mute : false),
		http_streaming_url: (state.core.http_streaming_url ? state.core.http_streaming_url : null),
		http_streaming_cachebuster: state.core.http_streaming_cachebuster,
		current_track: (state.core.current_track && state.core.tracks[state.core.current_track.uri] !== undefined ? state.core.tracks[state.core.current_track.uri] : null),
		next_track: (state.core.next_track_uri && state.core.tracks[state.core.next_track_uri] !== undefined ? state.core.tracks[state.core.next_track_uri] : null),
		radio_enabled: (state.ui.radio && state.ui.radio.enabled ? true : false),
		play_state: state.mopidy.play_state,
		time_position: state.mopidy.time_position,
		consume: state.mopidy.consume,
		repeat: state.mopidy.repeat,
		random: state.mopidy.random,
		volume: state.mopidy.volume,
		mute: state.mopidy.mute,
		sidebar_open: state.ui.sidebar_open
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		coreActions: bindActionCreators(coreActions, dispatch),
		uiActions: bindActionCreators(uiActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(PlaybackControls)