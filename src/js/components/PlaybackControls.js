
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'

import ProgressSlider from './ProgressSlider'
import VolumeControl from './VolumeControl'
import Dater from './Dater'
import ArtistSentence from './ArtistSentence'
import Thumbnail from './Thumbnail'

import * as mopidyActions from '../services/mopidy/actions'

class PlaybackControls extends React.Component{

	constructor(props) {
		super(props);
	}

	renderPlayButton(){
		var button = <a className="control play" onClick={() => this.props.mopidyActions.play()}><FontAwesome name="play" /> </a>
		if( this.props.play_state == 'playing' ){
			button = <a className="control play" onClick={() => this.props.mopidyActions.pause()}><FontAwesome name="pause" /> </a>
		}
		return button;
	}

	renderConsumeButton(){
		var button = <a className="control has-tooltip" onClick={() => this.props.mopidyActions.instruct('tracklist.setConsume', [true])}><FontAwesome name="fire" /><span className="tooltip">Consume</span></a>
		if( this.props.consume ){
			button = <a className="control active has-tooltip" onClick={() => this.props.mopidyActions.instruct('tracklist.setConsume', [false])}><FontAwesome name="fire" /><span className="tooltip">Consume</span></a>
		}
		return button;
	}

	renderRandomButton(){
		var button = <a className="control has-tooltip" onClick={() => this.props.mopidyActions.instruct('tracklist.setRandom', [true])}><FontAwesome name="random" /><span className="tooltip">Shuffle</span></a>
		if( this.props.random ){
			button = <a className="control active has-tooltip" onClick={() => this.props.mopidyActions.instruct('tracklist.setRandom', [false])}><FontAwesome name="random" /><span className="tooltip">Shuffle</span></a>
		}
		return button;
	}

	renderRepeatButton(){
		var button = <a className="control has-tooltip" onClick={() => this.props.mopidyActions.instruct('tracklist.setRepeat', [true])}><FontAwesome name="repeat" /><span className="tooltip">Repeat</span></a>
		if( this.props.repeat ){
			button = <a className="control active has-tooltip" onClick={() => this.props.mopidyActions.instruct('tracklist.setRepeat', [false])}><FontAwesome name="repeat" /><span className="tooltip">Repeat</span></a>
		}
		return button;
	}

	render(){
		var images = []
		if (this.props.current_track && this.props.current_track.album && this.props.current_track.album.images){
			images = this.props.current_track.album.images
		}

		return (
			<div className="playback-controls">
				
				<div className="current-track">
					<Thumbnail size="small" images={images} />
					<div className="title">
						{ this.props.current_track ? this.props.current_track.name : <span>-</span> }
					</div>
					<div className="artist">
						{ this.props.current_track ? <ArtistSentence artists={ this.props.current_track.artists } /> : <ArtistSentence /> }
					</div>
				</div>

				<section className="playback">
					<a className="control" onClick={() => this.props.mopidyActions.previous()}>
						<FontAwesome name="step-backward" />
					</a>
					{ this.renderPlayButton() }
					<a className="control" onClick={() => this.props.mopidyActions.stop()}>
						<FontAwesome name="stop" />
					</a>
					<a className="control" onClick={() => this.props.mopidyActions.next()}>
						<FontAwesome name="step-forward" />
					</a>
				</section>

				<section className="settings">
					{ this.renderConsumeButton() }
					{ this.renderRandomButton() }
					{ this.renderRepeatButton() }
				</section>

				<section className="progress">
					<ProgressSlider />
					<span className="current">{ this.props.time_position ? <Dater type="length" data={this.props.time_position} /> : '-' }</span>
					<span className="total">{ this.props.current_track ? <Dater type="length" data={this.props.current_track.length} /> : '-' }</span>
				</section>

				<section className="volume">
					<VolumeControl />
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
		current_track: (typeof(state.ui.current_track) !== 'undefined' && typeof(state.ui.tracks) !== 'undefined' && typeof(state.ui.tracks[state.ui.current_track.uri]) !== 'undefined' ? state.ui.tracks[state.ui.current_track.uri] : null),
		radio_enabled: (state.ui.radio && state.ui.radio.enabled ? true : false),
		play_state: state.mopidy.play_state,
		time_position: state.mopidy.time_position,
		consume: state.mopidy.consume,
		repeat: state.mopidy.repeat,
		random: state.mopidy.random
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		mopidyActions: bindActionCreators(mopidyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(PlaybackControls)