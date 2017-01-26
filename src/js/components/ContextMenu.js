
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'

import TrackList from './TrackList'
import * as helpers from '../helpers'
import * as uiActions from '../services/ui/actions'
import * as mopidyActions from '../services/mopidy/actions'
import * as spotifyActions from '../services/spotify/actions'

class ContextMenu extends React.Component{

	constructor(props) {
		super(props);
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

	handleScroll(){
		if( this.props.context_menu.show && this.props.context_menu.trigger == 'click' ){
			this.props.uiActions.hideContextMenu();
		}
	}

	handleClick(){
		if( this.props.context_menu.show && this.props.context_menu.trigger == 'click' ){
			this.props.uiActions.hideContextMenu();
		}
	}

	playQueueItem(){
		var tracks = this.props.context_menu.data.items;
		this.props.mopidyActions.changeTrack( tracks[0].tlid );		
		this.props.uiActions.hideContextMenu();
	}

	removeFromQueue(){
		var tracks = this.props.context_menu.data.items;
		var tracks_tlids = [];
		for( var i = 0; i < tracks.length; i++ ){
			tracks_tlids.push( tracks[i].tlid );
		}
		this.props.mopidyActions.removeTracks( tracks_tlids );
		this.props.uiActions.hideContextMenu();
	}

	playURIs(){
		this.props.mopidyActions.playURIs(this.props.context_menu.data.uris);
		this.props.uiActions.hideContextMenu();
	}

	playURIsNext(){
		this.props.mopidyActions.enqueueURIsNext(this.props.context_menu.data.uris);
		this.props.uiActions.hideContextMenu();
	}

	addToPlaylist(){
		this.props.uiActions.openModal( 'add_to_playlist', { tracks_uris: this.props.context_menu.data.uris } )
		this.props.uiActions.hideContextMenu();
	}

	addToQueue(){
		this.props.mopidyActions.enqueueURIs(this.props.context_menu.data.uris)
		this.props.uiActions.hideContextMenu()
	}

	addTracksToPlaylist( playlist_uri ){
		this.props.uiActions.addTracksToPlaylist( playlist_uri, this.props.context_menu.data.uris )
		this.props.uiActions.hideContextMenu();
	}

	removeFromPlaylist(){
		this.props.uiActions.removeTracksFromPlaylist( this.props.playlist.uri, this.props.context_menu.data.indexes )
		this.props.uiActions.hideContextMenu();
	}

	copyURIs(e){
		var temp = $("<input>");
		$("body").append(temp);
		temp.val(this.props.context_menu.data.uris.join(',')).select();
		document.execCommand("copy");
		temp.remove();

		this.props.uiActions.createNotification( "Copied "+this.props.context_menu.data.uris.length+" URIs" )
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

	getItems(trigger){
		switch (this.props.context_menu.context) {

			case 'queue':
				var items = [
					{ handleClick: 'playQueueItem', label: 'Play', icon: 'play' },
					{ handleClick: 'addToPlaylist', label: 'Add to playlist', icon: 'plus', playlists: true },
					{ handleClick: 'copyURIs', label: 'Copy URIs', icon: 'copy' },
					{ handleClick: 'removeFromQueue', label: 'Remove', icon: 'trash' }
				]
				break

			case 'editable-playlist':
				var items = [
					{ handleClick: 'playURIs', label: 'Play', icon: 'play' },
					{ handleClick: 'playURIsNext', label: 'Play next', icon: 'play' },
					{ handleClick: 'addToQueue', label: 'Add to queue', icon: 'plus' },
					{ handleClick: 'addToPlaylist', label: 'Add to playlist', icon: 'plus', playlists: true },
					{ handleClick: 'copyURIs', label: 'Copy URIs', icon: 'copy' },
					{ handleClick: 'removeFromPlaylist', label: 'Remove', icon: 'trash' }
				]
				break

			case 'album':
				var items = [
					{ handleClick: 'playURIs', label: 'Play', icon: 'play' },
					{ handleClick: 'playURIsNext', label: 'Play next', icon: 'play' },
					{ handleClick: 'addToQueue', label: 'Add to queue', icon: 'plus' },
					{ handleClick: 'copyURIs', label: 'Copy URIs', icon: 'copy' }
				]
				break

			default:
				var items = [
					{ handleClick: 'playURIs', label: 'Play', icon: 'play' },
					{ handleClick: 'playURIsNext', label: 'Play next', icon: 'play' },
					{ handleClick: 'addToQueue', label: 'Add to queue', icon: 'plus' },
					{ handleClick: 'addToPlaylist', label: 'Add to playlist', icon: 'plus', playlists: true },
					{ handleClick: 'copyURIs', label: 'Copy URIs', icon: 'copy' }
				]
				break
		}

		return items
	}

	renderItems(trigger){
		var items = this.getItems(trigger)

		var closeItem = (
			<span className="menu-item-wrapper cancel">
				<a className="menu-item" onClick={ (e) => this.closeAndDeselectTracks(e) }>
					<FontAwesome className="icon" fixedWidth name='close' />
					<span className="label">Cancel</span>
				</a>
			</span>
		)

		return (
			<div>
				{
					items.map((item, index) => {
						if( item.playlists ){
							return (
								<span key={item.handleClick} className="menu-item-wrapper has-submenu">
									<a className="menu-item" onClick={ (e) => this[item.handleClick](e) }>
										<FontAwesome className="icon" fixedWidth name={item.icon} />
										<span className="label">{ item.label }</span>
										<FontAwesome className="submenu-icon" name="caret-right" />
									</a>
									{ this.renderPlaylistSubmenu() }
								</span>
							)
						}else{
							return (
								<span className="menu-item-wrapper" key={item.handleClick}>
									<a className="menu-item" onClick={ (e) => this[item.handleClick](e) }>
										<FontAwesome className="icon" fixedWidth name={item.icon} />
										<span className="label">{ item.label }</span>
									</a>
								</span>
							)
						}
					})
				}
				{ trigger == 'touch' ? closeItem : null }
			</div>
		)
	}

	render(){
		if( !this.props.context_menu.show ) return null;

		var style = {
			left: this.props.context_menu.position_x,
			top: this.props.context_menu.position_y,
		}

		var className = 'context-menu'
		if (this.props.emulate_touch) {
			className += ' touch'
			var trigger = 'touch'
		} else {
			className += ' '+this.props.context_menu.trigger
			var trigger = this.props.context_menu.trigger
		}

		var items = this.getItems(trigger)
		var height = 0
		if (items) height = items.length * 34 // this is an approximation of how tall each menu item is

		if (this.props.context_menu.position_x > (window.innerWidth - 154)) className += ' right-align'
		if (this.props.context_menu.position_x > (window.innerWidth - 308)) className += ' right-align-submenu'
		if (this.props.context_menu.position_y > (window.innerHeight - height)){
			style.top = style.top - height
			className += ' bottom-align-submenu'
		}

		return (
			<div className={className} style={style}>
				{ this.renderItems(trigger) }
			</div>
		);
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		emulate_touch: state.ui.emulate_touch,
		context_menu: state.ui.context_menu,
		current_track: state.ui.current_track,
		current_tracklist: state.ui.current_tracklist,
		playlist: state.ui.playlist,
		playlists: state.ui.playlists
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(ContextMenu)