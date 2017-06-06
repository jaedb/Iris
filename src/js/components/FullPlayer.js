
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link } from 'react-router'

import FontAwesome from 'react-fontawesome'
import ProgressSlider from './ProgressSlider'
import ArtistSentence from './ArtistSentence'
import Thumbnail from './Thumbnail'
import Dater from './Dater'

import * as uiActions from '../services/ui/actions'
import * as mopidyActions from '../services/mopidy/actions'

class FullPlayer extends React.Component{

	constructor(props) {
		super(props);
	}

	handleContextMenu(e,item){
		e.preventDefault()
		var data = { 
			e: e,
			context: 'album',
			uris: [item.uri],
			items: [item]
		}
		this.props.uiActions.showContextMenu(data)
	}

	renderPlayButton(){
		var button = <a onClick={() => this.props.mopidyActions.play()}><FontAwesome name="play" /> </a>
		if( this.props.play_state == 'playing' ){
			button = <a onClick={() => this.props.mopidyActions.pause()}><FontAwesome name="pause" /> </a>
		}
		return button;
	}

	renderConsumeButton(){
		var button = <a className="has-tooltip" onClick={() => this.props.mopidyActions.instruct('tracklist.setConsume', [true])}><FontAwesome name="fire" /><span className="tooltip">Consume</span></a>
		if( this.props.consume ){
			button = <a className="active has-tooltip" onClick={() => this.props.mopidyActions.instruct('tracklist.setConsume', [false])}><FontAwesome name="fire" /><span className="tooltip">Consume</span></a>
		}
		return button;
	}

	renderRandomButton(){
		var button = <a className="has-tooltip" onClick={() => this.props.mopidyActions.instruct('tracklist.setRandom', [true])}><FontAwesome name="random" /><span className="tooltip">Shuffle</span></a>
		if( this.props.random ){
			button = <a className="active has-tooltip" onClick={() => this.props.mopidyActions.instruct('tracklist.setRandom', [false])}><FontAwesome name="random" /><span className="tooltip">Shuffle</span></a>
		}
		return button;
	}

	renderRepeatButton(){
		var button = <a className="has-tooltip" onClick={() => this.props.mopidyActions.instruct('tracklist.setRepeat', [true])}><FontAwesome name="repeat" /><span className="tooltip">Repeat</span></a>
		if( this.props.repeat ){
			button = <a className="active has-tooltip" onClick={() => this.props.mopidyActions.instruct('tracklist.setRepeat', [false])}><FontAwesome name="repeat" /><span className="tooltip">Repeat</span></a>
		}
		return button;
	}

	renderArtwork(){
		if( 
			!this.props.current_track || 
			!this.props.current_track.album || 
			!this.props.current_track.album.images ){
				return (
					<span className={this.props.radio_enabled ? 'artwork radio-enabled' : 'artwork'}>
						{this.props.radio_enabled ? <img className="radio-overlay" src="assets/radio-overlay.png" /> : null}
						<Thumbnail size="huge" />
					</span>
				)
		}

		var images = this.props.current_track.album.images
		if (typeof(this.props.tracks[this.props.current_track.uri]) !== 'undefined'){
			images = this.props.tracks[this.props.current_track.uri].album.images
		}

		var link = null
		if( this.props.current_track.album.uri ) link = '/album/'+this.props.current_track.album.uri
		return (
			<Link className={this.props.radio_enabled ? 'artwork radio-enabled' : 'artwork'} to={link} onContextMenu={e => this.handleContextMenu(e,this.props.current_track.album)}>
				{this.props.radio_enabled ? <img className="radio-overlay" src="assets/radio-overlay.png" /> : null}
				<Thumbnail size="huge" images={images} canZoom />
			</Link>
		)
	}

	render(){
		return (
			<div className="player">

				{ this.renderArtwork() }

				<div className="current-track">
					<div className="title">
						{this.props.current_track ? this.props.current_track.name : <span>-</span>}
					</div>
					{this.props.current_track ? <ArtistSentence artists={ this.props.current_track.artists } /> : <ArtistSentence />}
				</div>

				<div className="controls cf">
					<div className="pull-left">
						<a onClick={() => this.props.mopidyActions.previous()}>
							<FontAwesome name="step-backward" />
						</a>&nbsp;
						{ this.renderPlayButton() }
						<a onClick={() => this.props.mopidyActions.stop()}>
							<FontAwesome name="stop" />
						</a>&nbsp;
						<a onClick={() => this.props.mopidyActions.next()}>
							<FontAwesome name="step-forward" />
						</a>&nbsp;
					</div>
					<div className="pull-right">
						{ this.renderConsumeButton() }
						{ this.renderRandomButton() }
						{ this.renderRepeatButton() }
					</div>

				</div>

				<div className="progress">
					<ProgressSlider />	
					<span className="grey-text pull-left">{ this.props.time_position ? <Dater type="length" data={this.props.time_position} /> : '-' }</span>
					<span className="pull-right">{ this.props.current_track ? <Dater type="length" data={this.props.current_track.length} /> : '-' }</span>
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
		radio_enabled: (state.ui.radio && state.ui.radio.enabled ? true : false),
		tracks: state.ui.tracks,
		current_track: (typeof(state.ui.current_track) !== 'undefined' && typeof(state.ui.tracks) !== 'undefined' && typeof(state.ui.tracks[state.ui.current_track.uri]) !== 'undefined' ? state.ui.tracks[state.ui.current_track.uri] : null),
		play_state: state.mopidy.play_state,
		time_position: state.mopidy.time_position,
		consume: state.mopidy.consume,
		repeat: state.mopidy.repeat,
		random: state.mopidy.random
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(FullPlayer)