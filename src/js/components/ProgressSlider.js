
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link } from 'react-router'

import * as mopidyActions from '../services/mopidy/actions'

class ProgressSlider extends React.Component{

	constructor(props) {
		super(props);

		this.state = {
			animating: false
		}
	}

	componentDidMount(){
		var interval_counter = 0
		setInterval(() => {
			if( this.props.play_state == 'playing' ){

				// every 10 seconds get real position from Mopidy
				if( interval_counter % 10 == 0 ){
					this.updateProgress()
				}else{
					var time_position = this.props.time_position

					// only add 600ms every 1000ms as Mopidy's time tracker is a bit shit
					// TODO: Why does this kill UI?
					//this.props.mopidyActions.setTimePosition( time_position + 600 )					
				}

				interval_counter++
			}
		}, 1000);
	}

	updateProgress(){
		if( this.props.connected && this.props.play_state == 'playing' ){
			this.props.mopidyActions.getTimePosition()
		}
	}

	handleClick(e){
		var slider = e.target;
		if( slider.className != 'slider' ) slider = slider.parentElement;

		var sliderX = e.clientX - slider.getBoundingClientRect().left;
		var sliderWidth = slider.getBoundingClientRect().width;
		var percent = ( sliderX / sliderWidth ).toFixed(2);
		
		if( this.props.connected && this.props.current_tltrack ){
			var destination_time = this.props.current_track.length * percent
			this.props.mopidyActions.seek( destination_time )
			this.setState({ animating: false })
		}
	} 

	render(){
		var percent = 0
		if( this.props.connected && this.props.current_track ){
			percent = this.props.time_position / this.props.current_track.length
			percent = percent * 100;
			if( percent > 100 ) percent = 100
		}

		return (
			<div className='slider horizontal' onClick={ (e) => this.handleClick(e) } >
				<div className="track">
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
		current_track: state.ui.current_track,
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