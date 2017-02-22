
import React, { PropTypes } from 'react'
import { Link } from 'react-router'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'

import TrackList from './TrackList'
import * as helpers from '../helpers'
import * as uiActions from '../services/ui/actions'
import * as pusherActions from '../services/pusher/actions'
import * as mopidyActions from '../services/mopidy/actions'
import * as spotifyActions from '../services/spotify/actions'

class TouchContextMenu extends React.Component{

	constructor(props) {
		super(props);
	}

	close(){	
		this.props.uiActions.hideTouchContextMenu();
	}

	playQueueItem(){
		var tracks = this.props.menu.items
		this.props.mopidyActions.changeTrack( tracks[0].tlid )	
		this.close()
	}

	removeFromQueue(e){
		var tracks = this.props.menu.items
		var tracks_tlids = []
		for( var i = 0; i < tracks.length; i++ ){
			tracks_tlids.push( tracks[i].tlid );
		}
		this.props.mopidyActions.removeTracks( tracks_tlids )
		this.close()
	}

	playURIs(e){
		this.props.mopidyActions.playURIs(this.props.menu.uris, this.props.menu.tracklist_uri)
		this.close()
	}

	playURIsNext(e){
		this.props.mopidyActions.enqueueURIsNext(this.props.menu.uris, this.props.menu.tracklist_uri)
		this.close()
	}

	addToPlaylist(e){
		this.props.uiActions.openModal('add_to_playlist', { tracks_uris: this.props.menu.uris })
		this.close()
	}

	addToQueue(e){
		this.props.mopidyActions.enqueueURIs(this.props.menu.uris, this.props.menu.tracklist_uri)
		this.close()
	}

	removeFromPlaylist(e){
		this.props.uiActions.removeTracksFromPlaylist(this.props.menu.tracklist_uri, this.props.menu.indexes)
		this.close()
	}

	startRadio(e){
		this.props.pusherActions.startRadio(this.props.menu.uris)
		this.close()
	}

	getItems(){
		switch (this.props.menu.context) {

			case 'queue-track':
				var items = [
					{ handleClick: 'playQueueItem', label: 'Play', icon: 'play' },
					{ handleClick: 'addToPlaylist', label: 'Playlist', icon: 'plus' },
					{ handleClick: 'removeFromQueue', label: 'Remove', icon: 'trash' }
				]
				break

			case 'editable-playlist-track':
				var items = [
					{ handleClick: 'playURIs', label: 'Play', icon: 'play' },
					{ handleClick: 'playURIsNext', label: 'Next', icon: 'play' },
					{ handleClick: 'addToQueue', label: 'Queue', icon: 'plus' },
					{ handleClick: 'addToPlaylist', label: 'Playlist', icon: 'plus' },
					{ handleClick: 'startRadio', label: 'Start radio', icon: 'spotify' },
					{ handleClick: 'removeFromPlaylist', label: 'Remove', icon: 'trash' }
				]
				break

			default:
				var items = [
					{ handleClick: 'playURIs', label: 'Play', icon: 'play' },
					{ handleClick: 'playURIsNext', label: 'Next', icon: 'play' },
					{ handleClick: 'addToQueue', label: 'Queue', icon: 'plus' },
					{ handleClick: 'addToPlaylist', label: 'Playlist', icon: 'plus' },
					{ handleClick: 'startRadio', label: 'Start radio', icon: 'spotify' }
				]
				break
		}

		return items
	}

	renderItems(){
		var items = this.getItems()

		return (
			<div>
				{
					items.map((item, index) => {
						return (
							<span className="menu-item-wrapper" key={item.handleClick}>
								<a className="menu-item" onClick={e => this[item.handleClick]()}>
									<FontAwesome className="icon" fixedWidth name={item.icon} />
									<span className="label">{ item.label }</span>
								</a>
							</span>
						)
					})
				}
				<span className="menu-item-wrapper cancel" key="cancel">
					<a className="menu-item" onClick={e => this.close()}>
						<FontAwesome className="icon" fixedWidth name="close" />
						<span className="label">Cancel</span>
					</a>
				</span>
			</div>
		)
	}

	render(){
		if( !this.props.menu ) return null;

		return (
			<div className="touch-context-menu">
				{this.renderItems()}
			</div>
		);
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		menu: state.ui.touch_context_menu,
		current_track: state.ui.current_track,
		current_tracklist: state.ui.current_tracklist,
		playlist: state.ui.playlist,
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

export default connect(mapStateToProps, mapDispatchToProps)(TouchContextMenu)