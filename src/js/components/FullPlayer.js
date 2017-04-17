
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
		var button = <a onClick={() => this.props.mopidyActions.instruct('tracklist.setConsume', [true])}><FontAwesome name="fire" /></a>
		if( this.props.consume ){
			button = <a className="active" onClick={() => this.props.mopidyActions.instruct('tracklist.setConsume', [false])}><FontAwesome name="fire" /></a>
		}
		return button;
	}

	renderRandomButton(){
		var button = <a onClick={() => this.props.mopidyActions.instruct('tracklist.setRandom', [true])}><FontAwesome name="random" /></a>
		if( this.props.random ){
			button = <a className="active" onClick={() => this.props.mopidyActions.instruct('tracklist.setRandom', [false])}><FontAwesome name="random" /></a>
		}
		return button;
	}

	renderRepeatButton(){
		var button = <a onClick={() => this.props.mopidyActions.instruct('tracklist.setRepeat', [true])}><FontAwesome name="repeat" /></a>
		if( this.props.repeat ){
			button = <a className="active" onClick={() => this.props.mopidyActions.instruct('tracklist.setRepeat', [false])}><FontAwesome name="repeat" /></a>
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

		var link = null
		if( this.props.current_track.album.uri ) link = '/album/'+this.props.current_track.album.uri
		return (
			<Link className={this.props.radio_enabled ? 'artwork radio-enabled' : 'artwork'} to={link} onContextMenu={e => this.handleContextMenu(e,this.props.current_track.album)}>
				{this.props.radio_enabled ? <img className="radio-overlay" src="assets/radio-overlay.png" /> : null}
				<Thumbnail size="huge" images={this.props.current_track.album.images} canZoom />
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
		current_track: state.ui.current_track,
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