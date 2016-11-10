
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
		if( !this.props.dragger || !this.props.dragger.dragging ) return null;
		this.props.uiActions.dragMove( e )
	}

	handleMouseUp(e){
		if( !this.props.dragger || !this.props.dragger.dragging ) return null;
		this.props.uiActions.dragEnd( e )
	}

	render(){
		if( !this.props.dragger || !this.props.dragger.dragging ) return null;

		var style = {
			left: this.props.dragger.position_x,
			top: this.props.dragger.position_y,
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