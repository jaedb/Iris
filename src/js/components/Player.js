
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import FontAwesome from 'react-fontawesome'
import VolumeSlider from './VolumeSlider'
import * as actions from '../services/mopidy/actions'

class Player extends React.Component{

	constructor(props) {
		super(props);
	}

	renderTrackInFocus(){
		if( this.props.mopidy && this.props.mopidy.trackInFocus ){
			return (
				<div>
					<div>{ this.props.mopidy.trackInFocus.track.name }</div>
				</div>
			);
		}
		return null;
	}

	renderControls(){

		var playButton = <a onClick={() => this.props.actions.play()}><FontAwesome name="play" /> </a>
		if( this.props.mopidy.state == 'playing' ){
			playButton = <a onClick={() => this.props.actions.pause()}><FontAwesome name="pause" /> </a>
		}

		var consumeButton = <a onClick={() => this.props.actions.instruct('tracklist.setConsume', [true])}>Consume </a>
		if( this.props.mopidy.consume ){
			consumeButton = <a onClick={() => this.props.actions.instruct('tracklist.setConsume', [false])}>Un-Consume </a>
		}

		var randomButton = <a onClick={() => this.props.actions.instruct('tracklist.setRandom', [true])}>Random </a>
		if( this.props.mopidy.random ){
			randomButton = <a onClick={() => this.props.actions.instruct('tracklist.setRandom', [false])}>Un-Random </a>
		}

		var repeatButton = <a onClick={() => this.props.actions.instruct('tracklist.setRepeat', [true])}>Repeat </a>
		if( this.props.mopidy.repeat ){
			repeatButton = <a onClick={() => this.props.actions.instruct('tracklist.setRepeat', [false])}>Un-Repeat </a>
		}

		return (
			<div>
				{ this.renderTrackInFocus() }
				{ playButton }
				<a onClick={() => this.props.actions.previous()}>
					<FontAwesome name="step-backward" />
				</a>&nbsp;
				<a onClick={() => this.props.actions.next()}>
					<FontAwesome name="step-forward" />
				</a>&nbsp;
				{ consumeButton }
				{ randomButton }
				{ repeatButton }
				<VolumeSlider
					volume={ this.props.mopidy.volume } 
					onChange={(volume) => this.props.actions.instruct('playback.setVolume', { volume: volume })} />
			</div>
		);
	}

	render(){
		return (
			<div>
				{ this.renderControls() }
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
	return state;
}

const mapDispatchToProps = (dispatch) => {
	return {
		actions: bindActionCreators(actions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Player)