
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

		if( this.props.mopidy.connected && this.props.mopidy.state == 'playing' ){
			this.setState({ animating: true })
		}

		setInterval(() => {
			this.updateProgress()
		}, 10000);
	}

	componentWillReceiveProps( nextProps ){
		if( this.state.animating && nextProps.mopidy.state != 'playing' ){
			this.setState({ animating: false })
			console.log('stop animating')	// doesn't work?
		}
	}

	updateProgress(){
		if( this.props.mopidy.connected && this.props.mopidy.state == 'playing' ){
			this.setState({ animating: true })
			this.props.mopidyActions.getTimePosition()
		}
	}

	handleClick(e){
		var slider = e.target;
		if( slider.className != 'slider' ) slider = slider.parentElement;

		var sliderX = e.clientX - slider.getBoundingClientRect().left;
		var sliderWidth = slider.getBoundingClientRect().width;
		var percent = ( sliderX / sliderWidth ).toFixed(2);
		
		if( this.props.mopidy.connected && typeof(this.props.mopidy.current_tltrack) !== 'undefined' && typeof(this.props.mopidy.current_tltrack.track) !== 'undefined' ){
			var destination_time = this.props.mopidy.current_tltrack.track.length * percent
			this.props.mopidyActions.seek( destination_time )
			this.setState({ animating: false })
		}
	} 

	render(){
		var className = 'slider'
		if( this.state.animating ) className += ' animating'
		var percent = 0
		if( this.props.mopidy.connected && typeof(this.props.mopidy.current_tltrack) !== 'undefined' && typeof(this.props.mopidy.current_tltrack.track) !== 'undefined' ){
			percent = this.props.mopidy.time_position / this.props.mopidy.current_tltrack.track.length
			percent = percent * 100;
			if( percent > 100 ) percent = 100
		}

		return (
			<div className={className} onClick={ (e) => this.handleClick(e) } >
				<div className="progress" style={{ width: (percent)+'%' }}></div>
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

export default connect(mapStateToProps, mapDispatchToProps)(ProgressSlider)