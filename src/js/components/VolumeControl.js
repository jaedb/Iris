
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link } from 'react-router'
import FontAwesome from 'react-fontawesome'

import * as mopidyActions from '../services/mopidy/actions'
import * as uiActions from '../services/ui/actions'

class VolumeControl extends React.Component{

	constructor(props) {
		super(props)
	}

	handleClick(e){
		var slider = e.target;
		if( slider.className != 'slider' ) slider = slider.parentElement;

		var sliderX = e.clientX - slider.getBoundingClientRect().left;
		var sliderWidth = slider.getBoundingClientRect().width;
		var percent = Math.round(( sliderX / sliderWidth ) * 100);

		if (percent > 100){
			percent = 100
		} else if (percent < 0 ){
			percent = 0
		}

		this.props.mopidyActions.setVolume( percent )
	}

	handleWheel(e) {
		var percent = Math.round(this.props.volume - e.deltaY/8);

		if (percent > 100){
			percent = 100
		} else if (percent < 0 ){
			percent = 0
		}

		this.props.mopidyActions.setVolume( percent );

		e.preventDefault();
	}

	renderMuteButton(){
		if (this.props.mute){
			return (
				<a className="control has-tooltip" onClick={() => this.props.mopidyActions.setMute(false)}>
					<FontAwesome className="red-text" name="volume-down" />
					<span className="tooltip">Unmute</span>
				</a>
			)
		} else {
			return (
				<a className="control has-tooltip" onClick={() => this.props.mopidyActions.setMute(true)}>
					<FontAwesome className="muted" name="volume-off" />
					<span className="tooltip">Mute</span>
				</a>
			)
		}
	}

	render(){
		var className = "volume-control"
		if (this.props.mute){
			className += " muted"
		}
		return (
			<span className={className} onWheel={e => this.handleWheel(e)}>
				{this.renderMuteButton()}
				<div className="slider-wrapper">
					<div className="slider horizontal" onClick={e => this.handleClick(e)}>
						<div className="track">
							<div className="progress" style={{ width: this.props.volume+'%' }}></div>
						</div>
					</div>
				</div>
			</span>
		);
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		volume: state.mopidy.volume,
		mute: state.mopidy.mute
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(VolumeControl)
