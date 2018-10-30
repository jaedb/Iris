
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { createStore, bindActionCreators } from 'redux'

import Link from '../Link';

import * as helpers from '../../helpers'
import * as uiActions from '../../services/ui/actions'
import * as spotifyActions from '../../services/spotify/actions'

class FollowButton extends React.Component{

	constructor(props){
		super(props);
	}

	remove(){
		this.props.spotifyActions.following(this.props.uri, 'DELETE');
	}

	add(){
		this.props.spotifyActions.following(this.props.uri, 'PUT');
	}

	render(){
		if (!this.props.uri){
			return false;
		}

		var className = '';
		if (this.props.className){
			className += ' '+this.props.className;
		}

		if (!this.props.spotify_authorized){
			return <button className={className+' disabled'} onClick={e => this.props.uiActions.createNotification({content: 'You must authorize Spotify first', type: 'warning'})}>{this.props.addText}</button>
		} else if (this.props.is_following === true){
			return <button className={className+' destructive'} onClick={e => this.remove()}>{this.props.removeText}</button>
		} else {
			return <button className={className} onClick={e => this.add()}>{this.props.addText}</button>
		}
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		load_queue: state.ui.load_queue,
		spotify_authorized: state.spotify.authorization
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(FollowButton)