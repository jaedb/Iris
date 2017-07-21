
import React, { PropTypes } from 'react'
import { Link } from 'react-router'

import FontAwesome from 'react-fontawesome'
import ArtistSentence from './ArtistSentence'
import Dater from './Dater'
import * as helpers from '../helpers'

export default class Track extends React.Component{

	constructor(props) {
		super(props)

		this.state = {
			hover: false
		}

		this.drag_threshold = 5
		this.start_position = false
	}

	handleMouseDown(e){
		this.start_position = {
			x: e.pageX,
			y: e.pageY
		}
	}

	handleMouseMove(e){

		// No drag handling means no
		if (this.props.handleDrag === undefined){
			return false
		}

		if (this.start_position){
			let start_x = this.start_position.x
			let start_y = this.start_position.y
			let threshold = 5

			// Have we dragged outside of our threshold zone?
			if (e.pageX > start_x + threshold || e.pageX < start_x - threshold || e.pageY > start_y + threshold || e.pageY < start_y - threshold){

				// Handover to parent for dragging. We can unset all our behaviour now.
				this.props.handleDrag(e)
				this.start_position = false
			}
		}
	}

	handleMouseUp(e){
		if (this.props.dragger){
			e.preventDefault()

			if (this.props.handleDrop !== undefined){
				this.props.handleDrop(e)
			}
		} else {
			var target = $(e.target);
			if( !target.is('a') && target.closest('a').length <= 0 ){
				this.props.handleSelection(e)
				this.start_position = false
			}
		}
	}

	handleTouchStart(e){
		this.props.handleTouchDrag(e)
	}

	render(){
		if( !this.props.track ) return null

		var track = this.props.track
		var className = 'list-item track'
		if (this.props.selected) className += ' selected'
		if (this.props.can_sort) className += ' can-sort'
		if (track.type !== undefined) className += ' '+track.type
		if (track.playing) className += ' playing'
		if (this.state.hover) className += ' hover'
		
		var album = '-'
		if( track.album ){
			if( track.album.uri ){
				album = <Link to={global.baseURL+'album/'+track.album.uri}>{track.album.name}</Link>
			} else {
				album = <span>{track.album.name}</span>
			}
		}

		let track_columns = []

		if (track.type == 'history'){

			track_columns.push(
				<span className="col name">
					{track.name ? track.name : <span className="grey-text">{track.uri}</span>}
				</span>
			)
			track_columns.push(
				<span className="col source">
					{helpers.uriSource(track.uri)}
				</span>
			)
			track_columns.push(
				<span className="col played_at">
					{track.played_at ? <span><Dater type="ago" data={track.played_at} /> ago</span> : null}
				</span>
			)

		} else if (this.props.context == 'queue'){

			if (track.added_from && track.added_by){
				var type = (track.added_from ? helpers.uriType(track.added_from) : null)
				if (type == 'discover'){
					var link = <Link to={global.baseURL+'discover/recommendations/'+helpers.getFromUri('seeds',track.added_from)}>discover</Link>
				} else {
					var link = <Link to={global.baseURL+type+'/'+track.added_from}>{type}</Link>
				}
				var added = <span>{track.added_by} <span className="grey-text"> (from {link})</span></span>

			} else if (track.added_by){
				var added = track.added_by

			} else {
				var added = '-'
			}

			track_columns.push(
				<span className="col name" key="name">
					{track.name ? track.name : <span className="grey-text">{track.uri}</span>}
					{track.explicit ? <span className="flag dark">EXPLICIT</span> : null}
				</span>
			)
			if (this.props.show_source_icon){
				track_columns.push(
					<FontAwesome name={helpers.sourceIcon(track.uri)} className="source" key="source" fixedWidth />
				)
			}
			track_columns.push(
				<span className="col artists" key="artists">
					{track.artists ? <ArtistSentence artists={track.artists} /> : '-'}
				</span>
			)
			track_columns.push(
				<span className="col album" key="album">
					{album}
				</span>
			)
			track_columns.push(
				<span className="col added" key="added">
					{added}
				</span>
			)
			track_columns.push(
				<span className="col duration" key="duration">
					{track.duration_ms ? <Dater type="length" data={track.duration_ms} /> : null}
					{track.length ? <Dater type="length" data={track.length} /> : null}
				</span>
			)

		} else {

			track_columns.push(
				<span className="col name" key="name">
					{track.name ? track.name : <span className="grey-text">{track.uri}</span>}
					{track.explicit ? <span className="flag dark">EXPLICIT</span> : null}
				</span>
			)
			if (this.props.show_source_icon){
				track_columns.push(
					<FontAwesome name={helpers.sourceIcon(track.uri)} className="source" key="source" fixedWidth />
				)
			}
			track_columns.push(
				<span className="col artists" key="artists">
					{track.artists ? <ArtistSentence artists={track.artists} /> : '-'}
				</span>
			)
			track_columns.push(
				<span className="col album" key="album">
					{album}
				</span>
			)
			track_columns.push(
				<span className="col duration" key="duration">
					{track.duration_ms ? <Dater type="length" data={track.duration_ms} /> : null}
					{track.length ? <Dater type="length" data={track.length} /> : null}
				</span>
			)
		}

		if (this.props.slim_mode){

			// Select zone handles selection events only
			// We use onClick to capture touch as well as mouse events in one tidy parcel
			track_columns.push(
				<span 
					className="select-zone"
					key="select-zone"
					onClick={e => this.props.handleSelection(e)}>
						{this.props.selected ? <FontAwesome name="check" className="selected" fixedWidth /> : null}
				</span>
			)

			if (this.props.can_sort){
				track_columns.push(
					<span 
						className="drag-zone"
						key="drag-zone"
						onTouchStart={e => this.handleTouchStart(e)}>
							<FontAwesome name="bars" fixedWidth />
					</span>
				)
			}

			// No events attached directly to the track. Instead events are attached to
			// the appropriate select/drag zone sub-elements
			return (
				<div className={className}>
					{ track_columns }
				</div>
			)
		} else {
			return (
				<div
					className={className}
					onMouseEnter={e => this.setState({hover: true})}
					onMouseLeave={e => this.setState({hover: false})}
					onTouchEnd={e => this.handleTouchEnd(e)}			// When touch dragging is dropped on me
					onMouseDown={e => this.handleMouseDown(e)}			// Click (or potentially a mouse drag start)
					onMouseMove={e => this.handleMouseMove(e)}			// Any movement over me
					onMouseUp={e => this.handleMouseUp(e)}				// End of click, or potentially a dragging drop event
					onDoubleClick={e => this.props.handleDoubleClick(e)}
					onContextMenu={e => {e.preventDefault(); this.props.handleContextMenu(e)}}>
						{ track_columns }
				</div>
			)
		}
	}
}