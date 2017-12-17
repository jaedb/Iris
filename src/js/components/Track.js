
import React, { PropTypes } from 'react'
import { Link } from 'react-router'

import FontAwesome from 'react-fontawesome'
import ArtistSentence from './ArtistSentence'
import Dater from './Dater'
import URILink from './URILink'
import ContextMenuTrigger from './ContextMenuTrigger'

import * as helpers from '../helpers'

export default class Track extends React.Component{

	constructor(props){
		super(props)

		this.state = {
			hover: false
		}

		this.start_time = 0;
		this.start_position = false
	}

	handleMouseEnter(e){
		this.setState({hover: true});
	}

	handleMouseLeave(e){
		this.setState({hover: false});
	}

	handleMouseDown(e){
		var target = $(e.target);

		// Clicked a nested link (ie Artist name), so no dragging required
		if (target.is('a')){
			return false;
		}

		// Only listen for left mouse clicks
		if (e.button === 0){
			this.start_position = {
				x: e.pageX,
				y: e.pageY
			}

		// Not left click, then ensure no dragging
		} else {
			this.start_position = false
		}
	}

	handleMouseMove(e){
		var target = $(e.target);

		// No drag handling means NO
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
		var target = $(e.target);

		// Only listen for left clicks
		if (e.button === 0){
			if (this.props.dragger){
				e.preventDefault()

				if (this.props.handleDrop !== undefined){
					this.props.handleDrop(e);
				}
			} else {
				if (!target.is('a') && target.closest('a').length <= 0){
					this.props.handleSelection(e);
					this.start_position = false;
				}
			}

		// Not left click, then ensure no dragging
		} else {			
			this.start_position = false;
			return false;
		}
	}

	handleDoubleClick(e){
		this.props.handleDoubleClick(e);
	}

	handleTouchStart(e){
		var target = $(e.target);
		var timestamp = Math.floor(Date.now());

		// Clicked a nested link (ie Artist name), so no touch intervention required
		if (target.is('a')){
			return false;

		// We started a touchstart within 300ms ago, so handle as double-tap
		} else if ((timestamp - this.start_time) > 0 && (timestamp - this.start_time) <= 300){

			// Update our selection. By not passing touch = true selection will work like a regular click
			this.props.handleSelection(e);

			// Wait a moment to give Redux time to update our selected tracks
			// TODO: Use proper callback, rather than assuming a fixed period of time for store change
			setTimeout(() => {
					this.props.handleDoubleClick(e);
				}, 
				100
			);
			e.preventDefault();

		// Touch-drag zone
		} else if (target.hasClass('drag-zone')){
			this.props.handleTouchDrag(e);
			e.preventDefault();

		// Select zone
		} else if (target.hasClass('select-zone')){
			this.props.handleSelection(e, true);
			e.preventDefault();

		// Touch contextable
		} else if (target.hasClass('touch-contextable')){

			// Update our selection. By not passing touch = true selection will work like a regular click
			//this.props.handleSelection(e);
			this.handleContextMenu(e);
		}

		// Save touch start details
		this.start_time = timestamp;
		this.start_position = {
			x: e.touches[0].clientX,
			y: e.touches[0].clientY
		}

		return false;
	}

	handleTouchEnd(e){
		var target = $(e.target);
		var timestamp = Math.floor(Date.now());

		// Clicked a nested link (ie Artist name), so no dragging required
		if (!target.is('a')){
			e.preventDefault();
		}
	}

	handleContextMenu(e){
		e.preventDefault();
		e.stopPropagation();
		e.cancelBubble = true;
		this.props.handleContextMenu(e);
	}

	render(){
		if (!this.props.track){
			return null;
		}

		var track = this.props.track;
		var className = 'list-item track mouse-draggable mouse-selectable mouse-contextable';
		if (this.props.selected) className += ' selected';
		if (this.props.can_sort) className += ' can-sort';
		if (track.type !== undefined) className += ' '+track.type;
		if (track.playing) className += ' playing';
		if (this.state.hover) className += ' hover';
		
		var album = '-';
		if (track.album){
			if (track.album.uri){
				album = <URILink type="album" uri={track.album.uri}>{track.album.name}</URILink>;
			} else {
				album = <span>{track.album.name}</span>;
			}
		}

		let track_columns = [];
		let track_actions = [];

		if (track.type == 'history'){

			track_columns.push(
				<span className="col name" key="name">
					{track.name ? track.name : <span className="uri-placeholder grey-text">{track.uri}</span>}
				</span>
			)
			track_columns.push(
				<span className="col source" key="source">
					{helpers.uriSource(track.uri)}
				</span>
			)
			track_columns.push(
				<span className="col played_at" key="played_at">
					{track.played_at ? <span><Dater type="ago" data={track.played_at} /> ago</span> : null}
				</span>
			)

		} else if (this.props.context == 'queue'){

			if (track.added_from && track.added_by){
				var type = (track.added_from ? helpers.uriType(track.added_from) : null)
				if (type == 'discover'){
					var link = <URILink type="recommendations" uri={helpers.getFromUri('seeds',track.added_from)}>discover</URILink>
				} else {
					var link = <URILink type={type} uri={track.added_from}>{type}</URILink>
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
					{track.duration ? <Dater type="length" data={track.duration} /> : null}
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
					{track.duration ? <Dater type="length" data={track.duration} /> : '-'}
				</span>
			)
		}

		track_actions.push(
			<ContextMenuTrigger key="context" onTrigger={e => this.handleContextMenu(e)} />
		)

		if (this.props.mini_zones){

			// Select zone handles selection events only
			// We use onClick to capture touch as well as mouse events in one tidy parcel
			track_actions.push(
				<span 
					className="select-zone touch-selectable mouse-selectable"
					key="select-zone">
						{this.props.selected ? <FontAwesome name="check" className="selected" fixedWidth /> : null}
				</span>
			)

			if (this.props.can_sort){
				track_actions.push(
					<span 
						className="drag-zone touch-draggable mouse-draggable"
						key="drag-zone">
							<FontAwesome name="bars" fixedWidth />
					</span>
				)
			}
		}

		return (
			<div 
				className={className}
				onMouseEnter={e => this.handleMouseEnter(e)}
				onMouseLeave={e => this.handleMouseLeave(e)}
				onMouseDown={e => this.handleMouseDown(e)}
				onMouseUp={e => this.handleMouseUp(e)}
				onMouseMove={e => this.handleMouseMove(e)}

				onDoubleClick={e => this.handleDoubleClick(e)}
				onContextMenu={e => this.handleContextMenu(e)}

				onTouchStart={e => this.handleTouchStart(e)}
				onTouchEnd={e => this.handleTouchEnd(e)}>
					{track_actions}
					{track_columns}
			</div>
		);
	}
}