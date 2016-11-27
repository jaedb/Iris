
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import * as uiActions from '../services/ui/actions'
import * as mopidyActions from '../services/mopidy/actions'
import * as spotifyActions from '../services/spotify/actions'

class Dragger extends React.Component{

	constructor(props) {
		super(props);
		this.handleMouseMove = this.handleMouseMove.bind(this)
		this.handleMouseUp = this.handleMouseUp.bind(this)

		this.state = {
			active: false,
			position_x: 0,
			position_y: 0
		}
	}

	componentDidMount(){
		window.addEventListener("mousemove", this.handleMouseMove, false);
		window.addEventListener("mouseup", this.handleMouseUp, false);
	}

	componentWillUnmount(){
		window.removeEventListener("mousemove", this.handleMouseMove, false);
		window.removeEventListener("mouseup", this.handleMouseUp, false);
	}

	handleMouseMove(e){
		if( !this.props.dragger ) return null;

		var threshold = 10
		if(
			e.clientX > this.props.dragger.start_x + threshold || 
			e.clientX < this.props.dragger.start_x - threshold || 
			e.clientY > this.props.dragger.start_y + threshold || 
			e.clientY < this.props.dragger.start_y - threshold ){

			this.setState({
				position_x: e.clientX,
				position_y: e.clientY
			})

			var dropzones = document.getElementsByClassName('dropzone')
			for( var i = 0; i < dropzones.length; i++ ){
				dropzones[i].classList.remove('hover')
			}

			if( e.target.classList.contains('dropzone') && !e.target.classList.contains('hover') ){
				e.target.className += ' hover'
			}

			// if not already, activate
			if( !this.props.dragger.active ) this.props.uiActions.dragActive()
		}
	}

	handleMouseUp(e){
		if( !this.props.dragger ) return null;
		this.props.uiActions.dragEnd( e )
	}

	render(){
		if( !this.props.dragger || !this.props.dragger.active ) return null;

		var style = {
			left: this.state.position_x,
			top: this.state.position_y,
		}

		return (
			<div className="dragger" style={style}>
				Dragging { this.props.dragger.victims.length } things
			</div>
		);
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		dragger: state.ui.dragger
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Dragger)