
import React, { PropTypes } from 'react'
import { Link, hashHistory } from 'react-router'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'

import TrackList from './TrackList'
import * as helpers from '../helpers'
import * as uiActions from '../services/ui/actions'
import * as pusherActions from '../services/pusher/actions'
import * as mopidyActions from '../services/mopidy/actions'
import * as spotifyActions from '../services/spotify/actions'

class ContextMenu extends React.Component{

	constructor(props) {
		super(props)
		this.state = {
			submenu_expanded: false
		}
		this.handleScroll = this.handleScroll.bind(this)
		this.handleClick = this.handleClick.bind(this)
	}

	componentDidMount(){
		window.addEventListener("scroll", this.handleScroll, false)
		window.addEventListener("click", this.handleClick, false)
	}

	componentWillUnmount(){		
		window.removeEventListener("scroll", this.handleScroll, false)
		window.removeEventListener("click", this.handleClick, false)
	}

	componentWillReceiveProps( nextProps ){
		// if we've been given a menu object (ie activated) when we didn't have one prior
		if (nextProps.menu && !this.props.menu){			
			this.setState({ submenu_expanded: false })
			$('body').addClass('context-menu-open')

		// we DID have one prior, and now we don't
		} else if (this.props.menu && !nextProps.menu){
			$('body').removeClass('context-menu-open')
		}
	}

	handleScroll(e){
		if (this.props.menu){
			this.props.uiActions.hideContextMenu()
		}
	}

	handleClick(e){
		// if we click outside of the context menu or context menu trigger, kill it
		if ($(e.target).closest('.context-menu').length <= 0 && $(e.target).closest('.context-menu-trigger').length <= 0){
			this.props.uiActions.hideContextMenu()
		}
	}

	playQueueItem(){
		this.props.uiActions.hideContextMenu()
		var tracks = this.props.menu.items;
		this.props.mopidyActions.changeTrack( tracks[0].tlid )
	}

	removeFromQueue(){
		this.props.uiActions.hideContextMenu()
		var tracks = this.props.menu.items;
		var tracks_tlids = [];
		for( var i = 0; i < tracks.length; i++ ){
			tracks_tlids.push( tracks[i].tlid );
		}
		this.props.mopidyActions.removeTracks( tracks_tlids );
	}

	playURIs(){
		this.props.uiActions.hideContextMenu()
		this.props.mopidyActions.playURIs(this.props.menu.uris, this.props.menu.tracklist_uri)
	}

	playURIsNext(){
		this.props.uiActions.hideContextMenu()
		this.props.mopidyActions.enqueueURIsNext(this.props.menu.uris, this.props.menu.tracklist_uri)
	}

	addToPlaylist(){
		this.setState({ submenu_expanded: !this.state.submenu_expanded })
	}

	addToQueue(){
		this.props.uiActions.hideContextMenu()
		this.props.mopidyActions.enqueueURIs(this.props.menu.uris, this.props.menu.tracklist_uri)
	}

	addTracksToPlaylist(playlist_uri){
		this.props.uiActions.hideContextMenu()
		this.props.uiActions.addTracksToPlaylist(playlist_uri, this.props.menu.uris)
	}

	removeFromPlaylist(){
		this.props.uiActions.hideContextMenu()
		this.props.uiActions.removeTracksFromPlaylist(this.props.menu.tracklist_uri, this.props.menu.indexes)
	}

	startRadio(){
		this.props.uiActions.hideContextMenu()
		this.props.pusherActions.startRadio(this.props.menu.uris)
	}

	goToArtist(){
		if (!this.props.menu.items || this.props.menu.items.length <= 0 || !this.props.menu.items[0].artists || this.props.menu.items[0].artists.length <= 0){
			return null
		} else {
			this.props.uiActions.hideContextMenu()
			hashHistory.push( global.baseURL +'artist/'+ this.props.menu.items[0].artists[0].uri )
		}
	}

	goToUser(){
		if (!this.props.menu.items || this.props.menu.items.length <= 0){
			return null
		} else {
			this.props.uiActions.hideContextMenu()
			hashHistory.push( global.baseURL +'user/'+ this.props.menu.items[0].owner.uri )
		}
	}

	copyURIs(e){
		var temp = $("<input>");
		$("body").append(temp);
		temp.val(this.props.menu.uris.join(',')).select();
		document.execCommand("copy");
		temp.remove();

		this.props.uiActions.createNotification( "Copied "+this.props.menu.uris.length+" URIs" )
		this.props.uiActions.hideContextMenu()
	}

	closeAndDeselectTracks(){
		this.props.uiActions.hideContextMenu();
	}

	renderPlaylistSubmenu(){
		var playlists = []
		for (var uri in this.props.playlists){
			if (this.props.playlists[uri].can_edit) playlists.push( this.props.playlists[uri])
		}

		playlists = helpers.sortItems(playlists, 'name')

		return (			
			<div className="submenu">
				<span className="menu-item-wrapper">
					<a className="menu-item close-submenu" onClick={e => this.setState({submenu_expanded: false})}>
						<span className="label">
							<FontAwesome name='close' />
							&nbsp;
							Cancel
						</span>
					</a>
				</span>
				{
					playlists.map( playlist => {
						return (
							<span className="menu-item-wrapper" key={playlist.uri} >
								<a className="menu-item" onClick={ () => this.addTracksToPlaylist(playlist.uri) }>
									<span className="label">{ playlist.name }</span>
								</a>
							</span>
						)
					})
				}
			</div>
		)
	}

	getItems(){
		switch (this.props.menu.context) {

			case 'album':
				var items = [
					{ handleClick: 'playURIs', label: 'Play' },
					{ handleClick: 'playURIsNext', label: 'Play next' },
					{ handleClick: 'addToQueue', label: 'Add to queue' },
					{ handleClick: 'goToArtist', label: 'Go to artist' },
					// { handleClick: 'toggleFollow', label: 'Follow/unfollow' }, TODO
					{ handleClick: 'copyURIs', label: 'Copy URI' }
				]
				break

			case 'artist':
				var items = [
					//{ handleClick: 'toggleFollow', label: 'Follow/unfollow' }, TODO
					{ handleClick: 'startRadio', label: 'Start radio' },
					{ handleClick: 'copyURIs', label: 'Copy URI' }
				]
				break

			case 'playlist':
				var items = [
					{ handleClick: 'playURIs', label: 'Play' },
					{ handleClick: 'goToUser', label: 'Go to user' },
					// { handleClick: 'toggleFollow', label: 'Follow/unfollow' }, TODO
					{ handleClick: 'copyURIs', label: 'Copy URI' }
				]
				break

			case 'queue':
				var items = [
					{ handleClick: 'playQueueItem', label: 'Play' },
					{ handleClick: 'addToPlaylist', label: 'Add to playlist' },
					{ handleClick: 'copyURIs', label: 'Copy URIs' },
					{ handleClick: 'removeFromQueue', label: 'Remove' }
				]
				break

			case 'editable-playlist-track':
				var items = [
					{ handleClick: 'playURIs', label: 'Play' },
					{ handleClick: 'playURIsNext', label: 'Play next' },
					{ handleClick: 'addToQueue', label: 'Add to queue' },
					{ handleClick: 'addToPlaylist', label: 'Add to playlist' },
					{ handleClick: 'startRadio', label: 'Start radio' },
					{ handleClick: 'copyURIs', label: 'Copy URIs' },
					{ handleClick: 'removeFromPlaylist', label: 'Remove' }
				]
				break

			default:
				var items = [
					{ handleClick: 'playURIs', label: 'Play' },
					{ handleClick: 'playURIsNext', label: 'Play next' },
					{ handleClick: 'addToQueue', label: 'Add to queue' },
					{ handleClick: 'addToPlaylist', label: 'Add to playlist' },
					{ handleClick: 'startRadio', label: 'Start radio' },
					{ handleClick: 'copyURIs', label: 'Copy URIs' }
				]
				break
		}

		return items
	}

	renderTitle(){
		if (!this.props.menu.items || this.props.menu.items.length <= 0){
			return null
		}

		switch (this.props.menu.context){

			case 'artist':
			case 'album':
			case 'playlist':
				var item = this.props.menu.items[0]
				var style = null
				if (item && item.images){
					style = {
						backgroundImage: 'url('+helpers.sizedImages(item.images).medium+')'
					}
				}

				return (
					<Link className="title" to={global.baseURL+helpers.uriType(item.uri)+'/'+item.uri}>
						{style ? <div className="background" style={style}></div> : null}
						<div className="type">
							{helpers.uriSource(item.uri)}
							&nbsp;
							{this.props.menu.context}
						</div>
						<div className="text">{item.name}</div>
					</Link>
				)
				break

			default:
				return (
					<span className="title">
						<div className="type">
							{helpers.uriSource(this.props.menu.items[0].uri)}
							&nbsp;
							{this.props.menu.context}s
						</div>
						<div className="text">							
							{this.props.menu.items.length} items
						</div>
					</span>
				)
				break

		}
	}

	renderItems(){
		var items = this.getItems()

		return (
			<div>
				{
					items.map((item, index) => {
						if (item.handleClick == 'addToPlaylist'){
							return (
								<span key={item.handleClick} className="menu-item-wrapper has-submenu">
									<a className="menu-item" onClick={e => this[item.handleClick](e)}>
										<span className="label">{ item.label }</span>
										<FontAwesome className="submenu-icon" name='caret-right' />
									</a>
									{this.state.submenu_expanded ? this.renderPlaylistSubmenu() : null}
								</span>
							)
						}else{
							return (
								<span className="menu-item-wrapper" key={item.handleClick}>
									<a className="menu-item" onClick={e => this[item.handleClick](e)}>
										<span className="label">{ item.label }</span>
									</a>
								</span>
							)
						}
					})
				}
			</div>
		)
	}

	render(){
		if (!this.props.menu) return null;

		var style = {
			left: this.props.menu.position_x,
			top: this.props.menu.position_y,
		}

		var items = this.getItems()
		var height = 0
		if (items) height = items.length * 34 // this is an approximation of how tall each menu item is

		var className = "context-menu "+this.props.menu.context
		if (this.state.submenu_expanded) className += ' submenu-expanded'
		if (this.props.menu.position_x > (window.innerWidth - 154)) className += ' right-align'
		if (this.props.menu.position_x > (window.innerWidth - 308)) className += ' right-align-submenu'
		if (this.props.menu.position_y > (window.innerHeight - height)){
			style.top = style.top - height
			className += ' bottom-align-submenu'
		}

		return (
			<div className={className} style={style}>
				<div className="liner">
					{this.renderTitle()}
					{this.renderItems()}
				</div>
				<div className="background" onClick={e => this.props.uiActions.hideContextMenu()}></div>
			</div>
		);
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		menu: state.ui.context_menu,
		current_track: state.ui.current_track,
		current_tracklist: state.ui.current_tracklist,
		playlists: state.ui.playlists
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		pusherActions: bindActionCreators(pusherActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(ContextMenu)