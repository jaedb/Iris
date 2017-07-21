
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'

import Track from './Track'
import ContextMenuTrigger from './ContextMenuTrigger'

import * as helpers from '../helpers'
import * as mopidyActions from '../services/mopidy/actions'
import * as uiActions from '../services/ui/actions'

class TrackList extends React.Component{

	constructor(props) {
		super(props)

		this.touch_dragging = false
		this.handleKeyUp = this.handleKeyUp.bind(this)
		this.handleTouchMove = this.handleTouchMove.bind(this)
	}

	componentWillMount(){
		window.addEventListener("keyup", this.handleKeyUp, false)
		window.addEventListener("touchmove", this.handleTouchMove, false)
	}

	componentWillUnmount(){
		window.removeEventListener("keyup", this.handleKeyUp, false)
		window.removeEventListener("touchmove", this.handleTouchMove, false)
	}

	/**
	 * Figure out if our click/touch event is valid and we can act accordingly
	 *
	 * @param e = event obj
	 * @return string
	 **/
	triggerType(e){
		var target = $(e.target)

		// Wide screen, so no worries
		if (!this.props.slim_mode){
			return 'default'
		} else if (target.is('.select-zone') || target.closest('.select-zone').length > 0){
			return 'mobile'
		}

		return false
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
		key += '_'+track.uri
		key += '_'+(this.props.uri ? this.props.uri : 'none')
		key += '_'+(this.props.context ? this.props.context : 'none')
		return key
	}

	handleKeyUp(e){
		if (!this.selectedTracks()) return

		switch(e.keyCode){
			
			case 13: // enter
				 this.playTracks();
				break;
			
			case 46: // delete
				 this.removeTracks();
				break;
		}
	}

	handleTouchStart(e,index){
		var target = $(e.target)
		if (target.hasClass('drag-zone')){
			$('body').addClass('touch-dragging')
			this.touch_dragging = true
			e.preventDefault()
		}
	}

	handleTouchMove(e){
		if (this.touch_dragging){
			e.preventDefault()
			let touch = e.touches[0]
			let over = $(document.elementFromPoint(touch.clientX, touch.clientY))
			if (!over.is('.track')){
				over = over.closest('.track')
			}
			$(document).find('.touch-drag-hover').removeClass('touch-drag-hover')
			if (over.length > 0){
				over.addClass('touch-drag-hover')
			}
		}
	}

	handleTouchEnd(e,index){
		if (this.touch_dragging){
			let touch = e.changedTouches[0]
			let over = $(document.elementFromPoint(touch.clientX, touch.clientY))
			if (!over.is('.track')){
				over = over.closest('.track')
			}
			if (over.length > 0){
				let siblings = over.parent().children('.track')
				let dropped_at = siblings.index(over) - 1

				if (typeof(this.props.reorderTracks) !== 'undefined'){
					this.props.reorderTracks([index],dropped_at)
					this.render()
				}
			}

			$(document).find('.touch-drag-hover').removeClass('touch-drag-hover')
			$('body').removeClass('touch-dragging')
			this.touch_dragging = false
		} else {
			this.handleMouseDown(e,index)
		}

		// Prevent any event bubbling. This prevents clicks and mouse events
		// from also being fired
		e.preventDefault()
	}

	handleDoubleClick(e,track_key){
		if (this.props.context_menu) this.props.uiActions.hideContextMenu()
		this.playTracks()
	}

	handleMouseDown(e,track_key){
		if (this.props.context_menu) this.props.uiActions.hideContextMenu()

			
		this.toggleTrackSelections(e, track_key)

		if (this.props.dragger && this.props.dragger.active){
		
			// if this tracklist handles sorting, handle it
			if (typeof(this.props.reorderTracks) !== 'undefined'){
				var indexes = this.props.dragger.victims_indexes
				return this.props.reorderTracks( indexes, index )
			}
		}
/*
		// Regular clicking an un-selected element
		// This selects the track before we potentially drag
		switch (this.triggerType(e)){

			case 'mobile':
				// simple toggle
				var tracks = this.state.tracks
				tracks[index].selected = !tracks[index].selected
				this.setState({tracks: tracks, last_selected_track: index})
				break

			case 'default':
				if (!this.state.tracks[index].selected && !this.isRightClick(e) && !e.ctrlKey){
					this.toggleTrackSelections(e, index)
				}
				break
		}
		*/

		//this.props.uiActions.dragStart(e, this.props.context, this.props.uri, selected_tracks, this.tracksIndexes(selected_tracks))
	}

	handleMouseUp(e,track_key){
	}

	handleContextMenu(e, native_event = true){

		// Only enable direct context menu events in default (desktop) view
		// This prevents tap-and-hold functionality
		if (native_event && this.triggerType(e) != 'default'){
			return false
		}

		var selected_tracks = this.selectedTracks()
		var data = {
			e: e,
			context: (this.props.context ? this.props.context+'-track' : 'track'),
			tracklist_uri: (this.props.uri ? this.props.uri : null),
			items: selected_tracks,
			uris: helpers.asURIs(selected_tracks),
			indexes: this.tracksIndexes(selected_tracks)
		}
		this.props.uiActions.showContextMenu(data)

		if (!native_event){

			// Deselect all tracks
			this.props.uiActions.setSelectedTracks([])
		}
	}

	toggleTrackSelections(e,track_key){
		let selected_tracks = this.props.selected_tracks

		if (e.ctrlKey){

			// Already selected, so unselect it
			if (selected_tracks.includes(track_key)){
				var index = selected_tracks.indexOf(track_key)
				selected_tracks.splice(index,1)

			// Not selected, so add it
			} else {
				selected_tracks.push(track_key)
			}

		} else if (e.shiftKey){

			let last_selected_track_index = selected_tracks[selected_tracks.length-1].split('_')[0]
			let newly_selected_track_index = track_key.split('_')[0]

			// We've selected a track further down the list, 
			// so proceed normally
			if (last_selected_track_index < newly_selected_track_index){
				var start = last_selected_track_index
				var end = newly_selected_track_index

			// Selected a track up the list, so 
			// our last selected is the END of our range
			} else {
				var start = newly_selected_track_index
				var end = last_selected_track_index
			}

			if (start !== false && start >= 0 && end !== false && end >= 0){
				for (let i = start; i <= end; i++){
					selected_tracks.push(this.buildTrackKey(this.props.tracks[i], i))
				}
			}

		// Regular, unmodified left click
		} else {
			selected_tracks = [track_key]
		}

		this.props.uiActions.setSelectedTracks(selected_tracks)
	}

	isRightClick(e){
		if( 'which' in e ) 
			return e.which == 3
		if( 'button' in e ) 
			return e.button == 2
		return false
	}

	keyifyTracks(tracks, deselect = false){
		for( var i = 0; i < tracks.length; i++ ){
			var new_properties = {
				key: i+'_'+tracks[i].uri
			}
			if (deselect){
				new_properties.selected = false
			}
			tracks[i] = Object.assign(
				{}, 
				tracks[i], 
				new_properties
			)
		}
		return tracks
	}

	selectedTracks(indexes_only = false){
		if (!this.props.selected_tracks){
			return false
		}

		// Construct a basic track object, based on our unique track key
		// This is enough to perform interactions (dragging, selecting, etc)
		let response = []
		for (let i = 0; i < this.props.selected_tracks.length; i++){
			let track = this.props.selected_tracks[i].split('_')

			if (indexes_only){
				response.push(track[0])

			} else {
				response.push({
					index: track[0],
					uri: track[1],
					context: track[2],
					context_uri: track[3]
				})				
			}
		}

		return response
	}

	playTracks(){
		let selected_tracks = this.selectedTracks()

		// Our parent handles playing
		if (this.props.playTracks !== undefined){
			return this.props.playTracks( selected_tracks );
		}

		// Default to playing the URIs
		return this.props.mopidyActions.playURIs(helpers.asURIs(selected_tracks), this.props.uri )
	}

	removeTracks(){
		
		// Our parent handles removal
		if (this.props.removeTracks !== undefined){
			return this.props.removeTracks(this.selectedTracks(true))
		}

		// By default, do nothing
	}

	renderHeader(){
		if( this.props.noheader ) return null
		
		switch (this.props.context){

			case 'history':
				return (
					<div className="list-item header track">
						<span className="col name">Name</span>
						<span className="col source">Source</span>
						<span className="col played_at">Started playing</span>
					</div>
				)
				break

			case 'queue':
				return (
					<div className="list-item header track">
						<span className="col name">Name</span>
						<span className="col artists">Artists</span>
						<span className="col album">Album</span>
						<span className="col added">Added by</span>
						<span className="col duration">Length</span>
					</div>
				)
				break

			default:
				return (
					<div className="list-item header track">
						<span className="col name">Name</span>
						<span className="col artists">Artists</span>
						<span className="col album">Album</span>
						<span className="col duration">Length</span>
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
							let track_key = this.buildTrackKey(track, index)
							return (
								<Track
									show_source_icon={this.props.show_source_icon}
									key={track_key} 
									track={track} 
									context={this.props.context} 
									selected={this.props.selected_tracks.includes(track_key)} 
									handleDoubleClick={e => this.handleDoubleClick(e, track_key)}
									handleMouseUp={e => this.handleMouseUp(e, track_key)}
									handleMouseDown={e => this.handleMouseDown(e, track_key)}
									handleTouchStart={e => this.handleTouchStart(e, track_key)}
									handleTouchEnd={e => this.handleTouchEnd(e, track_key)}
									handleContextMenu={e => this.handleContextMenu(e)}
								/>
							)
						}
					)
				}
				{this.selectedTracks() ? <ContextMenuTrigger onTrigger={e => this.handleContextMenu(e, false)} /> : null}
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
		selected_tracks: state.ui.selected_tracks,
		slim_mode: state.ui.slim_mode,
		dragger: state.ui.dragger,
		current_track: state.ui.current_track,
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