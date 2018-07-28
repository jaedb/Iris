
import React from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';

import * as helpers from '../../helpers';
import * as mopidyActions from '../../services/mopidy/actions';

class ProgressSlider extends React.Component{

	constructor(props){
		super(props);
		
		this.handleChange = helpers.throttle(this.handleChange.bind(this), 250);
	}

	handleChange(value){
		this.props.mopidyActions.setTimePosition(this.props.current_track.duration * (value / 100));
	}

	render(){
		var percent = 0;
		if (this.props.connected && this.props.time_position && this.props.current_track){
			percent = this.props.time_position / this.props.current_track.duration;
			percent = percent * 100;
			if (percent > 1000){
				percent = 100;
			}
		}

		return (
			<div className={"progress slider horizontal "+this.props.play_state}>
				<div className="track">
					<input 
						type="range" 
						min="0" 
						max="100"
						value={percent}
						onChange={e => this.handleChange(parseInt(e.target.value))}
					/>
					<div className="progress" style={{ width: (percent)+'%' }}></div>
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
		current_track: (state.core.current_track && state.core.tracks[state.core.current_track.uri] !== undefined ? state.core.tracks[state.core.current_track.uri] : null),
		connected: state.mopidy.connected,
		time_position: state.mopidy.time_position,
		play_state: state.mopidy.play_state
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		mopidyActions: bindActionCreators(mopidyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(ProgressSlider)