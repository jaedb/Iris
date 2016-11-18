
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { Link } from 'react-router'
import { createStore, bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'

import * as helpers from '../helpers'
import * as spotifyActions from '../services/spotify/actions'

class FollowButton extends React.Component{

	constructor(props) {
		super(props);
	}

	componentDidMount(){
		if( this.props.spotify_authorized && this.props.uri ){
			this.props.spotifyActions.following(this.props.uri)			
		}
	}

	remove(){
		this.props.spotifyActions.following(this.props.uri, 'DELETE')
	}

	add(){
		this.props.spotifyActions.following(this.props.uri, 'PUT')
	}

	render(){
		if( !this.props.spotify_authorized || !this.props.uri ) return false
		var item = this.props[helpers.uriType( this.props.uri )]

		if( item.following === true ){
			return <button className="tertiary large" onClick={ () => this.remove() }>{ this.props.removeText }</button>
		}else{
			return <button className="tertiary large" onClick={ () => this.add() }>{ this.props.addText }</button>
		}
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		spotify_authorized: state.spotify.authorized,
		artist: state.ui.artist,
		user: state.ui.user,
		album: state.ui.album,
		playlist: state.ui.playlist
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(FollowButton)