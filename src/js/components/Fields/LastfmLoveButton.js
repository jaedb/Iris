
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { createStore, bindActionCreators } from 'redux'

import Link from '../Link';
import * as helpers from '../../helpers'
import * as uiActions from '../../services/ui/actions'
import * as lastfmActions from '../../services/lastfm/actions'

class FollowButton extends React.Component{

	constructor(props){
		super(props);
	}

	remove(){
		this.props.lastfmActions.unloveTrack(this.props.uri, this.props.artist, this.props.track);
	}

	add(){
		this.props.lastfmActions.loveTrack(this.props.uri, this.props.artist, this.props.track);
	}

	render(){
		if (!this.props.uri){
			return false;
		}

		var className = 'button';

		// Inherit passed-down classes
		if (this.props.className){
			className += ' '+this.props.className;
		}

		if (!this.props.lastfm_authorized){
			return <button className={className+' button--disabled'} onClick={e => this.props.uiActions.createNotification({content: 'You must authorize LastFM first', type: 'warning'})}>{this.props.addText}</button>
		} else if (this.props.is_loved && this.props.is_loved !== "0"){
			return <button className={className+' button--destructive'} onClick={e => this.remove()}>{this.props.removeText}</button>
		} else {
			return <button className={className} onClick={e => this.add()}>{this.props.addText}</button>
		}
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		load_queue: state.ui.load_queue,
		lastfm_authorized: state.lastfm.authorization
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		lastfmActions: bindActionCreators(lastfmActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(FollowButton)