
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
		var button = <a onClick={() => this.props.mopidyActions.play()}><FontAwesome name="play" /> </a>
		if( this.props.mopidy.state == 'playing' ){
			button = <a onClick={() => this.props.mopidyActions.pause()}><FontAwesome name="pause" /> </a>
		}
		return button;
	}

	render(){
		var mopidy_track = false;
		if( typeof(this.props.mopidy.current_tltrack) !== 'undefined' && typeof(this.props.mopidy.current_tltrack.track) !== 'undefined' ) mopidy_track = this.props.mopidy.current_tltrack;

		return (
			<div className="player">

				<div className="current-track">
					<div className="title">{ mopidy_track ? mopidy_track.track.name : null }</div>
					{ mopidy_track ? <ArtistSentence artists={ mopidy_track.track.artists } /> : null }
				</div>

				<div className="controls">
					{ this.renderPlayButton() }
					<a onClick={() => this.props.mopidyActions.next()}>
						<FontAwesome name="step-forward" />
					</a>&nbsp;
					<VolumeControl />
					<ProgressSlider />
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
	return state;
}

const mapDispatchToProps = (dispatch) => {
	return {
		mopidyActions: bindActionCreators(mopidyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(MiniPlayer)