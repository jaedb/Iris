
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

	renderControls(){

		var playButton = <a onClick={() => this.props.actions.instruct('playback.play')}><FontAwesome name="play" /> </a>
		if( this.props.mopidy.state == 'playing' ){
			playButton = <a onClick={() => this.props.actions.instruct('playback.pause')}><FontAwesome name="pause" /> </a>
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
				{ playButton }
				<a onClick={() => this.props.actions.instruct('playback.previous')}>
					<FontAwesome name="step-backward" />
				</a>&nbsp;
				<a onClick={() => this.props.actions.instruct('playback.next')}>
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