
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import FontAwesome from 'react-fontawesome'
import VolumeSlider from './VolumeSlider'
import ArtistSentence from './ArtistSentence'
import Thumbnail from './Thumbnail'

import * as mopidyActions from '../services/mopidy/actions'

class Player extends React.Component{

	constructor(props) {
		super(props);
	}

	renderTrack(){
		if( this.props.mopidy && this.props.mopidy.current_tltrack ){
			return (
				<div className="track-in-focus">
					{ this.props.spotify.track && !this.props.mini ? <Thumbnail size="large" images={this.props.spotify.track.album.images} /> : null }
					<div className="title">{ this.props.mopidy.current_tltrack.track.name }</div>
					<ArtistSentence artists={ this.props.mopidy.current_tltrack.track.artists } />
				</div>
			);
		}
		return null;
	}

	renderPlayButton(){
		var button = <a onClick={() => this.props.mopidyActions.play()}><FontAwesome name="play" /> </a>
		if( this.props.mopidy.state == 'playing' ){
			button = <a onClick={() => this.props.mopidyActions.pause()}><FontAwesome name="pause" /> </a>
		}
		return button;
	}

	renderConsumeButton(){
		if( this.props.mini ) return null;
		var button = <a onClick={() => this.props.mopidyActions.instruct('tracklist.setConsume', [true])}><FontAwesome name="fire" /></a>
		if( this.props.mopidy.consume ){
			button = <a className="active" onClick={() => this.props.mopidyActions.instruct('tracklist.setConsume', [false])}><FontAwesome name="fire" /></a>
		}
		return button;
	}

	renderRandomButton(){
		if( this.props.mini ) return null;
		var button = <a onClick={() => this.props.mopidyActions.instruct('tracklist.setRandom', [true])}><FontAwesome name="random" /></a>
		if( this.props.mopidy.random ){
			button = <a className="active" onClick={() => this.props.mopidyActions.instruct('tracklist.setRandom', [false])}><FontAwesome name="random" /></a>
		}
		return button;
	}

	renderRepeatButton(){
		if( this.props.mini ) return null;
		var button = <a onClick={() => this.props.mopidyActions.instruct('tracklist.setRepeat', [true])}><FontAwesome name="repeat" /></a>
		if( this.props.mopidy.repeat ){
			button = <a className="active" onClick={() => this.props.mopidyActions.instruct('tracklist.setRepeat', [false])}><FontAwesome name="repeat" /></a>
		}
		return button;
	}

	render(){
		return (
			<div className="player">

				{ this.renderTrack() }

				<div className="controls">
					{ this.renderPlayButton() }
					<a onClick={() => this.props.mopidyActions.previous()}>
						<FontAwesome name="step-backward" />
					</a>&nbsp;
					<a onClick={() => this.props.mopidyActions.next()}>
						<FontAwesome name="step-forward" />
					</a>&nbsp;
					{ this.renderConsumeButton() }
					{ this.renderRandomButton() }
					{ this.renderRepeatButton() }
					<VolumeSlider
						volume={ this.props.mopidy.volume } 
						onChange={(volume) => this.props.mopidyActions.instruct('playback.setVolume', { volume: volume })} />
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

export default connect(mapStateToProps, mapDispatchToProps)(Player)