
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link } from 'react-router'
import FontAwesome from 'react-fontawesome'

import ProgressSlider from './ProgressSlider'
import VolumeControl from './VolumeControl'
import ArtistSentence from './ArtistSentence'
import Thumbnail from './Thumbnail'

import * as mopidyActions from '../services/mopidy/actions'

class MiniPlayer extends React.Component{

	constructor(props) {
		super(props);
	}

	renderPlayButton(){
		var button = <a className="play" onClick={() => this.props.mopidyActions.play()}><FontAwesome name="play" /> </a>
		if( this.props.play_state == 'playing' ){
			button = <a className="play" onClick={() => this.props.mopidyActions.pause()}><FontAwesome name="pause" /> </a>
		}
		return button;
	}

	render(){
		return (
			<div className="player mini-player">

				<div className="current-track">
					<div className="title">{ this.props.current_track ? this.props.current_track.name : <span>-</span> }</div>
					{ this.props.current_track ? <ArtistSentence artists={ this.props.current_track.artists } /> : <ArtistSentence /> }
				</div>

				<div className="controls">
					{ this.renderPlayButton() }
					<a className="next" onClick={() => this.props.mopidyActions.next()}>
						<FontAwesome name="step-forward" />
					</a>&nbsp;
					<VolumeControl />
				</div>

				<ProgressSlider />
				
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
		play_state: state.mopidy.play_state
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		mopidyActions: bindActionCreators(mopidyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(MiniPlayer)