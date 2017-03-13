
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'

import Thumbnail from '../components/Thumbnail'
import PlaylistGrid from '../components/PlaylistGrid'
import FollowButton from '../components/FollowButton'
import LazyLoadListener from '../components/LazyLoadListener'
import Header from '../components/Header'
import ContextMenuTrigger from '../components/ContextMenuTrigger'

import * as helpers from '../helpers'
import * as mopidyActions from '../services/mopidy/actions'
import * as spotifyActions from '../services/spotify/actions'

class User extends React.Component{

	constructor(props) {
		super(props);
	}

	componentDidMount(){
		this.loadUser();
	}

	componentWillReceiveProps( nextProps ){
		if( nextProps.params.uri != this.props.params.uri ) this.loadUser( nextProps )
	}

	loadUser( props = this.props ){
		if (!props.user) this.props.spotifyActions.getUser( props.params.uri )
	}

	loadMore(){
		this.props.spotifyActions.getURL( this.props.user.playlists_more, 'SPOTIFY_USER_PLAYLISTS_LOADED', this.props.params.uri )
	}

	isMe(){
		if( !this.props.spotify_authorized ) return null
		return helpers.getFromUri('userid',this.props.params.uri) == this.props.me.id
	}

	render(){
		if (this.props.user){
			var playlists = []
			if (this.props.user.playlists_uris){
				for (var i = 0; i < this.props.user.playlists_uris.length; i++){
					var uri = this.props.user.playlists_uris[i]
					if (this.props.playlists.hasOwnProperty(uri)){
						playlists.push(this.props.playlists[uri])
					}
				}
			}

			return (
				<div className="view user-view">
				
					<Header icon="play" title="User" />

					<div className="intro">
						<Thumbnail circle={true} size="medium" images={ this.props.user.images } />

						<h1>{ this.props.user.display_name ? this.props.user.display_name : this.props.user.id }</h1>

						<ul className="details">
							{this.isMe() ? <li>You</li> : null}
							<li>{this.props.user.playlists_total ? this.props.user.playlists_total.toLocaleString() : 0} playlists</li>
							<li>{this.props.user.followers.total.toLocaleString()} followers</li>
						</ul>

						<div className="actions">
							<FollowButton className="secondary" uri={this.props.params.uri} addText="Follow" removeText="Unfollow" />
						</div>
					</div>
					<div className="main">

						<section className="grid-wrapper">
							<PlaylistGrid playlists={playlists} />
							<LazyLoadListener enabled={this.props.user.playlists_more} loadMore={ () => this.loadMore() }/>
						</section>
						
					</div>
				</div>
			)
		} else {

			return (
				<div className="view user-view">				
					<Header icon="play" title="User" />
					<div className="intro">
						<Thumbnail circle size="medium" images={[]} />
						<h1><span className="placeholder"></span></h1>
						<div className="actions">
							<button className="placeholder"></button>
						</div>
					</div>
				</div>
			);
		}

	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		spotify_authorized: state.spotify.authorized,
		me: state.spotify.me,
		playlists: state.ui.playlists,
		user: (state.ui.users && typeof(state.ui.users[ownProps.params.uri]) !== 'undefined' ? state.ui.users[ownProps.params.uri] : false ),
		users: state.ui.users
	};
}

const mapDispatchToProps = (dispatch) => {
	return {
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(User)