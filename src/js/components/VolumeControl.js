
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

		var sliderY = e.clientY - slider.getBoundingClientRect().top;
		var sliderHeight = slider.getBoundingClientRect().height;
		var percent = 100 - parseInt( sliderY / sliderHeight * 100 );

		this.props.mopidyActions.setVolume( percent )
	}

	render(){
		var className = "volume-control"
		return (
			<span className={className}>

				<a className="modal-trigger" onClick={() => this.props.uiActions.openModal('volume')}>
					{this.props.mute ? <FontAwesome className="muted" name="volume-off" /> : <FontAwesome name="volume-down" />}
				</a>

				<span className="hover-trigger">
					<a onClick={() => this.props.mopidyActions.setMute(!this.props.mute)}>
						{this.props.mute ? <FontAwesome className="muted" name="volume-off" /> : <FontAwesome name="volume-down" />}
					</a>
					<div className="slider-wrapper">
						<div className={this.props.mute ? "slider vertical disabled" : "slider vertical"} onClick={ (e) => this.handleClick(e) } >
							<div className="track">
								<div className="progress" style={{ height: this.props.volume+'%' }}></div>
							</div>
						</div>
					</div>
				</span>

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