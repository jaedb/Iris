
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { Link } from 'react-router'
import { bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'

import ProgressSlider from './ProgressSlider'
import VolumeControl from './VolumeControl'
import Dater from './Dater'
import ArtistSentence from './ArtistSentence'
import Thumbnail from './Thumbnail'
import Icon from './Icon'

import * as uiActions from '../services/ui/actions'
import * as mopidyActions from '../services/mopidy/actions'

class PlaybackControls extends React.Component{

	constructor(props){
		super(props)

		this.state = {
			expanded: false
		}
	}

	renderPlayButton(){
		var button = <a className="control play" onClick={() => this.props.mopidyActions.play()}><Icon name="play" /></a>
		if (this.props.play_state == 'playing'){
			button = <a className="control play" onClick={() => this.props.mopidyActions.pause()}><Icon name="pause" /></a>
		}
		return button;
	}

	renderConsumeButton(){
		var button = <a className="control has-tooltip" onClick={() => this.props.mopidyActions.instruct('tracklist.setConsume', [true])}><FontAwesome name="fire" /><span className="tooltip">Consume</span></a>
		if (this.props.consume){
			button = <a className="control active has-tooltip" onClick={() => this.props.mopidyActions.instruct('tracklist.setConsume', [false])}><FontAwesome name="fire" /><span className="tooltip">Consume</span></a>
		}
		return button;
	}

	renderRandomButton(){
		var button = <a className="control has-tooltip" onClick={() => this.props.mopidyActions.instruct('tracklist.setRandom', [true])}><FontAwesome name="random" /><span className="tooltip">Shuffle</span></a>
		if (this.props.random){
			button = <a className="control active has-tooltip" onClick={() => this.props.mopidyActions.instruct('tracklist.setRandom', [false])}><FontAwesome name="random" /><span className="tooltip">Shuffle</span></a>
		}
		return button;
	}

	renderRepeatButton(){
		var button = <a className="control has-tooltip" onClick={() => this.props.mopidyActions.instruct('tracklist.setRepeat', [true])}><FontAwesome name="repeat" /><span className="tooltip">Repeat</span></a>
		if (this.props.repeat){
			button = <a className="control active has-tooltip" onClick={() => this.props.mopidyActions.instruct('tracklist.setRepeat', [false])}><FontAwesome name="repeat" /><span className="tooltip">Repeat</span></a>
		}
		return button;
	}

	handleThumbnailClick(e){
		e.preventDefault();
		this.props.uiActions.openModal('kiosk_mode');
	}

	render(){
		var images = false
		if (this.props.current_track && this.props.current_track.images){
			images = this.props.current_track.images
		}

		return (
			<div className={(this.state.expanded ? "expanded playback-controls" : "playback-controls")}>
				
				<div className="current-track">
					<div className="thumbnail-wrapper" onClick={e => this.handleThumbnailClick(e)}>
						<Thumbnail size="small" images={images} />
					</div>
					<div className="title">
						{ this.props.current_track ? this.props.current_track.name : <span>-</span> }
					</div>
					<div className="artist">
						{ this.props.current_track ? <ArtistSentence artists={ this.props.current_track.artists } /> : <ArtistSentence /> }
					</div>
				</div>

				<section className="playback">
					<a className="control previous" onClick={() => this.props.mopidyActions.previous()}>
						<Icon name="back" />
					</a>
					{ this.renderPlayButton() }
					<a className="control stop" onClick={() => this.props.mopidyActions.stop()}>
						<Icon name="stop" />
					</a>
					<a className="control next" onClick={() => this.props.mopidyActions.next()}>
						<Icon name="skip" />
					</a>
				</section>

				<section className="settings">
					{this.renderConsumeButton()}
					{this.renderRandomButton()}
					{this.renderRepeatButton()}
				</section>

				<section className="progress">
					<ProgressSlider />
					<span className="current">{ this.props.time_position ? <Dater type="length" data={this.props.time_position} /> : '-' }</span>
					<span className="total">{ this.props.current_track ? <Dater type="length" data={this.props.current_track.length} /> : '-' }</span>
				</section>

				<section className="volume">
					<VolumeControl />
				</section>

				<section className="triggers">
					<a className="control expanded-controls" onClick={() => this.setState({expanded: !this.state.expanded})}>
						{this.state.expanded ? <FontAwesome name="chevron-down" /> : <FontAwesome name="chevron-up" />}
					</a>
					<a className={"control sidebar-toggle"+(this.props.sidebar_open ? ' open' : '')} onClick={() => this.props.uiActions.toggleSidebar()}>
						<FontAwesome className="open" name="bars" />
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
		current_track: (state.core.current_track !== undefined && state.core.tracks !== undefined && state.core.tracks[state.core.current_track] !== undefined ? state.core.tracks[state.core.current_track] : null),
		radio_enabled: (state.ui.radio && state.ui.radio.enabled ? true : false),
		play_state: state.mopidy.play_state,
		time_position: state.mopidy.time_position,
		consume: state.mopidy.consume,
		repeat: state.mopidy.repeat,
		random: state.mopidy.random,
		sidebar_open: state.ui.sidebar_open
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(PlaybackControls)