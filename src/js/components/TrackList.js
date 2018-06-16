
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import Track from './Track'
import ContextMenuTrigger from './ContextMenuTrigger'

import * as helpers from '../helpers'
import * as mopidyActions from '../services/mopidy/actions'
import * as uiActions from '../services/ui/actions'

class TrackList extends React.Component{

	constructor(props){
		super(props)

		this.touch_dragging_tracks_keys = false

		this.handleKeyDown = this.handleKeyDown.bind(this)
		this.handleTouchMove = this.handleTouchMove.bind(this)
		this.handleTouchEnd = this.handleTouchEnd.bind(this)
	}

	componentWillMount(){
		window.addEventListener("keydown", this.handleKeyDown, false)
		window.addEventListener("touchmove", this.handleTouchMove, false)
		window.addEventListener("touchend", this.handleTouchEnd, false)
	}

	componentWillUnmount(){
		window.removeEventListener("keydown", this.handleKeyDown, false)
		window.removeEventListener("touchmove", this.handleTouchMove, false)
		window.removeEventListener("touchend", this.handleTouchEnd, false)
	}

	handleKeyDown(e){

		// When we're focussed on certian elements (like form input fields), don't fire any shortcuts
		var ignoreNodes = ['INPUT', 'TEXTAREA']
		if (ignoreNodes.indexOf(e.target.nodeName) > -1){
			return false
		}

		var tracks_keys = this.digestTracksKeys()

		switch(e.keyCode){			
			case 13: // enter
				if (tracks_keys && tracks_keys.length > 0){
					this.playTracks();
				}
				break;
			
			case 46: // delete
				if (tracks_keys && tracks_keys.length > 0){
					this.removeTracks();
				}
				break;
			
			case 65: // a
				if ((e.ctrlKey || e.metaKey)){

					e.preventDefault();

					// Select all our tracks
					var all_tracks = []
					for (var i = 0; i < this.props.tracks.length; i++){
						all_tracks.push(this.buildTrackKey(this.props.tracks[i], i))
					}
					this.props.uiActions.setSelectedTracks(all_tracks)

					return false
				}
				break;
		}
	}

	handleDrag(e,track_key){

		let selected_tracks = []

		// Dragging a non-selected track. We need to deselect everything
		// else and select only this track
		if (!this.props.selected_tracks.includes(track_key)){
			this.props.uiActions.setSelectedTracks([track_key])
			selected_tracks = this.digestTracksKeys([track_key])
		} else {
			selected_tracks = this.digestTracksKeys()
		}

		let selected_tracks_indexes = helpers.arrayOf('index',selected_tracks)

		this.props.uiActions.dragStart(
			e, 
			this.props.context, 
			this.props.uri, 
			selected_tracks, 
			selected_tracks_indexes
		)
	}

	handleDrop(e,track_key){
		if (this.props.dragger && this.props.dragger.active){
		
			// if this tracklist handles sorting, handle it
			if (this.props.reorderTracks !== undefined){
				let indexes = this.props.dragger.victims_indexes
				let tracks = this.digestTracksKeys([track_key])
				return this.props.reorderTracks(indexes, tracks[0].index)
			}
		}
		this.touch_dragging_tracks_keys = false
	}

	handleTouchDrag(e,track_key){
		let selected_tracks = []

		// Drag initiated on a selected track
		if (this.props.selected_tracks.includes(track_key)){

			// They're all dragging
			this.touch_dragging_tracks_keys = this.props.selected_tracks

		// Not already selected
		} else {
			this.touch_dragging_tracks_keys = [track_key]
			this.props.uiActions.setSelectedTracks([track_key])
		}
	}

	handleTouchMove(e){
		if (this.touch_dragging_tracks_keys){
			
			let touch = e.touches[0];
			let over = $(document.elementFromPoint(touch.clientX, touch.clientY));
			if (!over.is('.track')){
				over = over.closest('.track');
			}
			$(document).find('.touch-drag-hover').removeClass('touch-drag-hover');
			if (over.length > 0){
				over.addClass('touch-drag-hover');
			}

	        e.returnValue = false;
	        e.cancelBubble = true;
            e.preventDefault();
            e.stopPropagation();
	        return false;
		}
	}

	handleTouchEnd(e){
		if (this.touch_dragging_tracks_keys){
			let touch = e.changedTouches[0];
			let over = $(document.elementFromPoint(touch.clientX, touch.clientY));
			if (!over.is('.track')){
				over = over.closest('.track');
			}
			if (over.length > 0){
				let siblings = over.parent().children('.track');
				let dropped_at = siblings.index(over) - 1;

				if (this.props.reorderTracks !== undefined){
					this.props.reorderTracks(helpers.arrayOf('index',this.digestTracksKeys()),dropped_at);
					this.props.uiActions.setSelectedTracks([]);
				}
			}

			$(document).find('.touch-drag-hover').removeClass('touch-drag-hover');
			$('body').removeClass('touch-dragging');
		}
		
		this.touch_dragging_tracks_keys = false;
	}

	handleTap(e, track_key){
		this.updateSelection(e, track_key, true);
	}

	handleDoubleTap(e,track_key){
		this.playTracks([track_key]);
		this.updateSelection(e, track_key);
	}

	handleClick(e, track_key){
		this.updateSelection(e, track_key);
	}

	handleDoubleClick(e,track_key){
		if (this.props.context_menu){
			this.props.uiActions.hideContextMenu();
		}
		this.playTracks([track_key]);
		this.updateSelection(e, track_key);
	}

	handleContextMenu(e, track_key = null){

		// Do our best to stop any flow-on events
		e.preventDefault();
		e.stopPropagation();
		e.cancelBubble = true;

		let selected_tracks = this.props.selected_tracks;

		// Not already selected, so select it prior to triggering menu
		if (track_key && !selected_tracks.includes(track_key)){
			selected_tracks = [track_key];
			this.props.uiActions.setSelectedTracks(selected_tracks);
		}

		let selected_tracks_digested = this.digestTracksKeys(selected_tracks);
		let selected_tracks_uris = helpers.arrayOf('uri',selected_tracks_digested);
		let selected_tracks_indexes = helpers.arrayOf('index',selected_tracks_digested);

		let data = {
			e: e,
			context: (this.props.context ? this.props.context+'-track' : 'track'),
			tracklist_uri: (this.props.uri ? this.props.uri : null),
			items: selected_tracks_digested,
			uris: selected_tracks_uris,
			indexes: selected_tracks_indexes
		}

		this.props.uiActions.showContextMenu(data);
	}

	updateSelection(e, track_key, touched = false){
		let selected_tracks = this.props.selected_tracks;

		if ((e.ctrlKey || e.metaKey) || touched){

			// Already selected, so unselect it
			if (selected_tracks.includes(track_key)){
				var index = selected_tracks.indexOf(track_key);
				selected_tracks.splice(index, 1);

			// Not selected, so add it
			} else {
				selected_tracks.push(track_key);
			}

		} else if (e.shiftKey){

			let last_selected_track = this.digestTracksKeys(selected_tracks[selected_tracks.length-1]);
			let last_selected_track_index = last_selected_track.index;
			let newly_selected_track = this.digestTracksKeys(track_key);
			let newly_selected_track_index = newly_selected_track.index;

			// We've selected a track further down the list, 
			// so proceed normally
			if (last_selected_track_index < newly_selected_track_index){
				var start = last_selected_track_index + 1;
				var end = newly_selected_track_index;

			// Selected a track up the list, so 
			// our last selected is the END of our range
			} else {
				var start = newly_selected_track_index;
				var end = last_selected_track_index - 1;
			}

			if (start !== false && start >= 0 && end !== false && end >= 0){
				for (let i = start; i <= end; i++){
					selected_tracks.push(this.buildTrackKey(this.props.tracks[i], i));
				}
			}

		// Regular, unmodified left click
		} else {
			selected_tracks = [track_key];
		}

		this.props.uiActions.setSelectedTracks(selected_tracks);
	}

	isRightClick(e){
		if ('which' in e ){
			return e.which == 3
		} else if ('button' in e){
			return e.button == 2;
		}
		return false;
	}

	playTracks(tracks_keys = null){
		if (tracks_keys !== null){
			var selected_tracks = this.digestTracksKeys(tracks_keys);
		} else {
			var selected_tracks = this.digestTracksKeys();
		}
		var selected_tracks_indexes = helpers.arrayOf('index', selected_tracks);

		if (selected_tracks.length <= 0){
			return this.props.uiActions.createNotification({content: 'No tracks selected', type: 'bad'});
		}

		// Our parent handles playing
		if (this.props.playTracks !== undefined){
			return this.props.playTracks(selected_tracks);

		// Default to playing the URIs
		} else {
			let selected_tracks_uris = helpers.arrayOf('uri',selected_tracks);
			return this.props.mopidyActions.playURIs(selected_tracks_uris, this.props.uri);
		}
	}

	removeTracks(){
		
		// Our parent handles removal
		if (this.props.removeTracks !== undefined){
			let selected_tracks = this.digestTracksKeys()
			let selected_tracks_indexes = helpers.arrayOf('index',selected_tracks)
			return this.props.removeTracks(selected_tracks_indexes)
		}

		// By default, do nothing
	}


	/**
	 * Build the track key
	 * This is our unique reference to a track in a particular tracklist
	 *
	 * @param track = Track obj
	 * @param index = int (position of track in tracklist)
	 * @return string
	 **/
	buildTrackKey(track, index){
		let key = index
		key += '@@'+(track.tlid ? track.tlid : 'none')
		key += '@@'+track.uri
		key += '@@'+(this.props.uri ? this.props.uri : 'none')
		key += '@@'+(this.props.context ? this.props.context : 'none')
		return key
	}


	/**
	 * Digest our selected tracks
	 *
	 * @param tracks = mixed (defaults to stored value)
	 * @param indexex_only = boolean (do we just want an array of indexes)
	 * @return mixed
	 **/
	digestTracksKeys(keys = this.props.selected_tracks, indexes_only = false){
		if (!keys){
			return false
		}

		// Accommodate a single key
		let singleton = false
		if (!(keys instanceof Array)){
			singleton = true
			keys = [keys]
		}

		// Construct a basic track object, based on our unique track key
		// This is enough to perform interactions (dragging, selecting, etc)
		let array = []
		for (let i = 0; i < keys.length; i++){
			let key = keys[i].split('@@')

			if (indexes_only){
				array.push(key[0])

			} else {
				array.push({
					index: parseInt(key[0]),
					tlid: parseInt(key[1]),
					uri: key[2],
					context: key[3],
					context_uri: key[4]
				})				
			}
		}

		if (singleton && array.length > 0){
			return array[0]
		} else {
			return array
		}
	}

	renderHeader(){
		if (this.props.noheader ) return null
		
		switch (this.props.context){

			case 'history':
				return (
					<div className="list-item header track">
						<div className="liner">
							<span className="col name">Name</span>
							<span className="col artists">Artists</span>
							<span className="col album">Album</span>
							<span className="col played_at">Started playing</span>
						</div>
					</div>
				)
				break

			case 'queue':
				return (
					<div className="list-item header track">
						<div className="liner">
							<span className="col name">Name</span>
							<span className="col artists">Artists</span>
							<span className="col album">Album</span>
							<span className="col added">Added by</span>
							<span className="col duration">Duration</span>
						</div>
					</div>
				)
				break

			default:
				return (
					<div className="list-item header track">
						<div className="liner">
							<span className="col name">Name</span>
							<span className="col artists">Artists</span>
							<span className="col album">Album</span>
							<span className="col duration">Duration</span>
							<span className="col popularity"></span>
						</div>
					</div>
				)
		}
	}

	render(){
		if (!this.props.tracks || Object.prototype.toString.call(this.props.tracks) !== '[object Array]' ) return null

		var className = 'list track-list '+this.props.context
		if (this.props.className){
			className += ' '+this.props.className
		}

		return (
			<div className={className}>
				{ this.renderHeader() }
				{
					this.props.tracks.map(
						(track, index) => {
							let track_key = this.buildTrackKey(track, index);
							track.key = track_key;
							return (
								<Track
									show_source_icon={this.props.show_source_icon}
									key={track_key} 
									mini_zones={this.props.slim_mode || helpers.isTouchDevice()}
									track={track} 
									context={this.props.context} 
									can_sort={this.props.context == 'queue' || this.props.context == 'editable-playlist'} 
									selected={this.props.selected_tracks.includes(track_key)} 
									dragger={this.props.dragger} 
									handleClick={(e) => this.handleClick(e, track_key)}
									handleDoubleClick={e => this.handleDoubleClick(e, track_key)}
									handleContextMenu={e => this.handleContextMenu(e, track_key)}
									handleDrag={e => this.handleDrag(e, track_key)}
									handleDrop={e => this.handleDrop(e, track_key)}
									handleTap={e => this.handleTap(e, track_key)}
									handleDoubleTap={e => this.handleDoubleTap(e, track_key)}
									handleTouchDrag={e => this.handleTouchDrag(e, track_key)}
								/>
							)
						}
					)
				}
			</div>
		);
	}
}


/**
 * Export our component
 *
 * We also integrate our global store, using connect()
 **/

const mapStateToProps = (state, ownProps) => {
	return {
		slim_mode: state.ui.slim_mode,
		selected_tracks: state.ui.selected_tracks,
		dragger: state.ui.dragger,
		current_track: state.core.current_track,
		context_menu: state.ui.context_menu
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		uiActions: bindActionCreators(uiActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(TrackList)