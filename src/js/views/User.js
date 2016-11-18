
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'

import Thumbnail from '../components/Thumbnail'
import PlaylistGrid from '../components/PlaylistGrid'
import FollowButton from '../components/FollowButton'
import LazyLoadListener from '../components/LazyLoadListener'

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
		this.props.spotifyActions.getUser( props.params.uri )
	}

	loadMore(){
		if( !this.props.user.playlists_more ) return
		this.props.spotifyActions.getURL( this.props.user.playlists_more, 'SPOTIFY_USER_PLAYLISTS_LOADED_MORE' )
	}

	renderMeFlag(){
		if( !this.props.spotify_authorized ) return null

		if( helpers.getFromUri('userid',this.props.params.uri) == this.props.me.id ){
			return <span className="flag blue"><FontAwesome name="star" />&nbsp;You</span>
		}else{
			return null
		}
	}

	render(){
		if( !this.props.user ) return null

		return (
			<div className="view user-view">
				<div className="intro">
					<Thumbnail circle={true} size="medium" images={ this.props.user.images } />

					<h1>{ this.props.user.display_name ? this.props.user.display_name : this.props.user.id }{ this.renderMeFlag() }</h1>

					<div className="actions">
						<FollowButton uri={this.props.params.uri} addText="Follow" removeText="Unfollow" />
					</div>

					<ul className="details">
						<li>{ this.props.user.playlists_total.toLocaleString() } playlists</li>
						<li>{ this.props.user.followers.total.toLocaleString() } followers</li>
					</ul>
				</div>
				<div className="main">

					<section className="grid-wrapper">
						{ this.props.user.playlists ? <PlaylistGrid playlists={ this.props.user.playlists } /> : null }
						<LazyLoadListener loadMore={ () => this.loadMore() }/>
					</section>
					
				</div>
			</div>
		);
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		spotify_authorized: state.spotify.authorized,
		me: state.spotify.me,
		user: state.ui.user
	};
}

const mapDispatchToProps = (dispatch) => {
	return {
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(User)