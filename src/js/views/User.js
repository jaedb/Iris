
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import Thumbnail from '../components/Thumbnail'
import PlaylistGrid from '../components/PlaylistGrid'
import FollowButton from '../components/Fields/FollowButton'
import LazyLoadListener from '../components/LazyLoadListener'
import Parallax from '../components/Parallax'
import ContextMenuTrigger from '../components/ContextMenuTrigger'
import Icon from '../components/Icon'

import * as helpers from '../helpers'
import * as coreActions from '../services/core/actions'
import * as uiActions from '../services/ui/actions'
import * as mopidyActions from '../services/mopidy/actions'
import * as spotifyActions from '../services/spotify/actions'

class User extends React.Component{

	constructor(props){
		super(props);
	}

	componentDidMount(){
		this.setWindowTitle();
		this.props.coreActions.loadUser(this.props.params.uri);
		this.props.coreActions.loadUserPlaylists(this.props.params.uri);
	}

	componentWillReceiveProps(nextProps){
		if (nextProps.params.uri != this.props.params.uri){
			this.props.coreActions.loadUser(nextProps.params.uri);
			this.props.coreActions.loadUserPlaylists(this.props.params.uri);
		}

		if (!this.props.user && nextProps.user){
			this.setWindowTitle(nextProps.user);
		}
	}

	setWindowTitle(user = this.props.user){
		if (user){
			this.props.uiActions.setWindowTitle(user.name+" (user)");
		} else{
			this.props.uiActions.setWindowTitle("User");
		}
	}

	loadMore(){
		this.props.spotifyActions.getMore(
			this.props.user.playlists_more,
			{
				parent_type: 'user',
				parent_key: this.props.params.uri,
				records_type: 'playlist'
			}
		);
	}

	isMe(){
		let userid = helpers.getFromUri('userid',this.props.params.uri);
		return (this.props.me && this.props.me.id && this.props.me.id == userid);
	}

	render(){
		var user_id = helpers.getFromUri('userid',this.props.params.uri);

		if (!this.props.user){
			if (helpers.isLoading(this.props.load_queue,['spotify_users/'+user_id,'spotify_users/'+user_id+'/playlists/?'])){
				return (
					<div className="body-loader loading">
						<div className="loader"></div>
					</div>
				)
			} else {
				return null;
			}
		}

		var user = helpers.collate(this.props.user, {playlists: this.props.playlists});

		if (user && user.images){
			var image = helpers.sizedImages(user.images).huge;
		} else {
			var image = null;
		}

		return (
			<div className="view user-view">
				<div className="intro">
					<Parallax image={image} theme={this.props.theme} />
					<div className="liner">
						<h1>{user.name}</h1>
						<h2>
							<ul className="details">
								{user.playlists_total ? <li>{user.playlists_total ? user.playlists_total.toLocaleString() : 0} playlists</li> : null}
								{user.followers ? <li>{user.followers.toLocaleString()} followers</li> : null}
								{this.isMe() ? <li><span className="blue-text">You</span></li> : null}
							</ul>
						</h2>
						<div className="actions">
							<FollowButton className="primary" uri={user.uri} addText="Follow" removeText="Unfollow" />
						</div>
					</div>
				</div>

				<div className="content-wrapper">
					<section className="grid-wrapper">
						<h4>Playlists</h4>
						<PlaylistGrid playlists={user.playlists} />
						<LazyLoadListener loading={user.playlists_more} loadMore={() => this.loadMore()} />
					</section>
				</div>
			</div>
		)
	}
}

const mapStateToProps = (state, ownProps) => {
	var uri = ownProps.params.uri;
	return {
		me: state.spotify.me,
		theme: state.ui.theme,
		load_queue: state.ui.load_queue,
		spotify_authorized: state.spotify.authorization,
		playlists: state.core.playlists,
		user: (state.core.users[uri] !== undefined ? state.core.users[uri] : false)
	};
}

const mapDispatchToProps = (dispatch) => {
	return {
		coreActions: bindActionCreators(coreActions, dispatch),
		uiActions: bindActionCreators(uiActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(User)
