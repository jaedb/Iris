
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
		var item = {}
		switch (helpers.uriType( this.props.uri )){
			case 'artist':
				item = this.props.artists[this.props.uri]
				break
			case 'user':
				item = this.props.users[this.props.uri]
				break
			case 'album':
				item = this.props.albums[this.props.uri]
				break
			case 'playlist':
				item = this.props.playlists[this.props.uri]
				break
		}

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
		artists: state.ui.artists,
		users: state.ui.users,
		albums: state.ui.albums,
		playlists: state.ui.playlists
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(FollowButton)