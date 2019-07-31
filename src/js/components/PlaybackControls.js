
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';

import Link from './Link';
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
			expanded: false,
			current_track: null,
			transition_track: null,
			transition_direction: null
		}
	}

	componentDidMount(){
		if (this.props.http_streaming_enabled){

			// Bust our cache, and by consequence, play our stream
			this.props.coreActions.cachebustHttpStream();
		}

		if (this.props.current_track){			
			this.setState({current_track: this.props.current_track});
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

		// Started a track or changed to no track
		if ((!this.props.current_track && nextProps.current_track) || (this.props.current_track && !nextProps.current_track)){
			this.setState({current_track: nextProps.current_track});
		}

		// Direct swap-out of a track (with no 'null' intermediate state)
		// This is precautionary and I've never been able to trigger it, but it's a good safety
		if (this.props.current_track && nextProps.current_track && this.props.current_track.uri !== nextProps.current_track.uri){
			this.setState({current_track: nextProps.current_track});
		}

		// Images have just loaded
		// A bit niggly to have to deeply check this...
		if (this.props.current_track && nextProps.current_track){

			if ((this.props.current_track.images && !nextProps.current_track.images) || (!this.props.current_track.images && nextProps.current_track.images)){
				this.setState({current_track: nextProps.current_track});
			}
		}
	}

	handleTouchStart(e){
		var target = $(e.target);
		var timestamp = Math.floor(Date.now());

		// Save touch start details
		this.start_time = timestamp;
		this.start_position = {
			x: e.touches[0].clientX
		}

		return false;
	}

	handleTouchEnd(e){
		var target = $(e.target);
		var timestamp = Math.floor(Date.now());
		var tap_distance_threshold = 10;		// Max distance (px) between touchstart and touchend to qualify as a tap
		var tap_time_threshold = 200;			// Max time (ms) between touchstart and touchend to qualify as a tap
		var end_position = {
			x: e.changedTouches[0].clientX
		}

		// Too long between touchstart and touchend
		if (this.start_time + tap_time_threshold < timestamp){
			return false;
		}

		// Make sure there's enough distance between start and end before we handle
		// this event as a 'tap'
		if (this.start_position.x + tap_distance_threshold > end_position.x &&
			this.start_position.x - tap_distance_threshold < end_position.x){

			// Scroll to top (without smooth_scroll)
			helpers.scrollTo(null, false);
			this.props.history.push('/queue');
		} else {

			// Swipe to the left = previous track
			if (this.start_position.x < end_position.x){
				this.setTransition('previous');
				this.props.mopidyActions.previous();

			// Swipe to the right = skip track
			} else if (this.start_position.x > end_position.x){
				this.setTransition('next');
				this.props.mopidyActions.next();
			}
		}

		this.end_time = timestamp;
		e.preventDefault();
	}

	setTransition(direction){
		this.setState({
			current_track: null,
			transition_track: this.state.current_track,
			transition_direction: direction
		});

		// Allow time for the animation to complete, then remove
		// the transitioning track from state
		var self = this;
		setTimeout(() => {
				this.setState({
					transition_track: null,
					transition_direction: null
				});
			},
			250
		);
	}

	renderPlayButton(){
		var button = <button className="control play" onClick={() => this.props.mopidyActions.play()}><Icon name="play_circle_filled" type="material" /></button>
		if (this.props.play_state == 'playing'){
			button = <button className="control play" onClick={() => this.props.mopidyActions.pause()}><Icon name="pause_circle_filled" type="material" /></button>
		}
		return button;
	}

	renderConsumeButton(){
		var button = <button className="control tooltip" onClick={() => this.props.mopidyActions.setConsume(true)}><Icon name="restaurant" type="material" /><span className="tooltip__content">Consume</span></button>
		if (this.props.consume){
			button = <button className="control control--active tooltip" onClick={() => this.props.mopidyActions.setConsume(false)}><Icon name="restaurant" type="material" /><span className="tooltip__content">Consume</span></button>
		}
		return button;
	}

	renderRandomButton(){
		var button = <button className="control tooltip" onClick={() => this.props.mopidyActions.setRandom(true)}><Icon name="shuffle" type="material" /><span className="tooltip__content">Shuffle</span></button>
		if (this.props.random){
			button = <button className="control control--active tooltip" onClick={() => this.props.mopidyActions.setRandom(false)}><Icon name="shuffle" type="material" /><span className="tooltip__content">Shuffle</span></button>
		}
		return button;
	}

	renderRepeatButton(){
		var button = <button className="control tooltip" onClick={() => this.props.mopidyActions.setRepeat(true)}><Icon name="repeat" /><span className="tooltip__content">Repeat</span></button>
		if (this.props.repeat){
			button = <button className="control control--active tooltip" onClick={() => this.props.mopidyActions.setRepeat(false)}><Icon name="repeat" /><span className="tooltip__content">Repeat</span></button>
		}
		return button;
	}

	render(){
		var images = false
		if (this.state.current_track && this.state.current_track.images){
			images = this.state.current_track.images
		}

		return (
			<div className={(this.state.expanded ? "playback-controls--expanded playback-controls" : "playback-controls")}>

				{this.props.next_track && this.props.next_track.images ? <Thumbnail className="hide" size="large" images={this.props.next_track.images} /> : null}
				
				{this.state.transition_track && this.state.transition_direction ? <div 
					className={"current-track current-track__transition current-track__transition--"+this.state.transition_direction}>
						<div className="text">
							<div className="title">
								{this.state.transition_track.name}
							</div>
							<div className="artist">
								<ArtistSentence artists={this.state.transition_track.artists} nolinks={true} />
							</div>
						</div>
					</div> : null}
				
				<div 
					className={this.state.transition_track && this.state.transition_direction ? "current-track current-track--transitioning" : "current-track"}
					onTouchStart={e => this.handleTouchStart(e)}
					onTouchEnd={e => this.handleTouchEnd(e)}
					tabIndex="-1">
						<Link className="thumbnail-wrapper" to={'/kiosk-mode'} tabIndex="-1">
							<Thumbnail size="small" images={images} />
						</Link>
						<div className="text">
							<div className="title">
								{this.state.current_track ? this.state.current_track.name : <span>-</span>}
							</div>
							<div className="artist">
								{this.state.current_track ? <ArtistSentence artists={this.state.current_track.artists} /> : <ArtistSentence />}
							</div>
						</div>
				</div>

				<section className="playback">
					{ this.renderPlayButton() }
					<button className="control previous" onClick={() => this.props.mopidyActions.previous()}>
						<Icon name="navigate_before" type="material" />
					</button>
					<button className="control next" onClick={() => this.props.mopidyActions.next()}>
						<Icon name="navigate_next" type="material" />
					</button>
				</section>

				<section className="progress">
					<ProgressSlider />
					<span className="current">{ this.props.time_position ? <Dater type="length" data={this.props.time_position} /> : '-' }</span>
					<span className="total">{ this.state.current_track ? <Dater type="length" data={this.state.current_track.duration} /> : '-' }</span>
				</section>

				<section className="settings">
					{this.renderConsumeButton()}
					{this.renderRandomButton()}
					{this.renderRepeatButton()}
					<OutputControl force_expanded={this.state.expanded} />
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
					<button className="control expanded-controls" onClick={e => this.setState({expanded: !this.state.expanded})}>
						{this.state.expanded ? <Icon name="expand_more" type="material" /> : <Icon name="expand_less" type="material" />}
					</button>
					<button className={"control sidebar-toggle"+(this.props.sidebar_open ? ' open' : '')} onClick={e => this.props.uiActions.toggleSidebar()}>
						<Icon className="open" name="menu" type="material" />
					</button>
				</section>
				
			</div>
		);
	}
}

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
		sidebar_open: state.ui.sidebar_open,
		slim_mode: state.ui.slim_mode
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