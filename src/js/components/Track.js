
import React, { PropTypes } from 'react'
import Link from './Link'

import Icon from './Icon'
import ArtistSentence from './ArtistSentence'
import Dater from './Dater'
import URILink from './URILink'
import ContextMenuTrigger from './ContextMenuTrigger'
import Popularity from './Popularity'
import ErrorBoundary from './ErrorBoundary'

import * as helpers from '../helpers'

export default class Track extends React.Component{

	constructor(props){
		super(props)

		this.state = {
			hover: false
		}

		this.start_time = 0;
		this.end_time = 0;
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
					this.props.handleClick(e);
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

		// Touch-drag zone
		if (target.hasClass('drag-zone')){
			this.props.handleTouchDrag(e);
			e.preventDefault();
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
		var tap_distance_threshold = 10;		// Max distance (px) between touchstart and touchend to qualify as a tap
		var tap_time_threshold = 200;			// Max time (ms) between touchstart and touchend to qualify as a tap
		var end_position = {
			x: e.changedTouches[0].clientX,
			y: e.changedTouches[0].clientY
		}

		// Too long between touchstart and touchend
		if (this.start_time + tap_time_threshold < timestamp){
			return false;
		}

		// Make sure there's enough distance between start and end before we handle
		// this event as a 'tap'
		if (this.start_position.x + tap_distance_threshold > end_position.x &&
			this.start_position.x - tap_distance_threshold < end_position.x &&
			this.start_position.y + tap_distance_threshold > end_position.y &&
			this.start_position.y - tap_distance_threshold < end_position.y){

			// Clicked a nested link (ie Artist name), so no dragging required
			if (!target.is('a')){
				e.preventDefault();
			}

			// Context trigger
			if (target.hasClass('touch-contextable')){

				// Update our selection. By not passing touch = true selection will work like a regular click
				//this.props.handleSelection(e);
				this.props.handleContextMenu(e);
				return false;
			}

			// We received a touchend within 300ms ago, so handle as double-tap
			if ((timestamp - this.end_time) > 0 && (timestamp - this.end_time) <= 300){
				this.props.handleDoubleTap(e);
				e.preventDefault();
				return false;
			}

			this.props.handleTap(e);
		}

		this.end_time = timestamp;
	}

	render(){
		if (!this.props.track){
			return null;
		}

		var track = this.props.track;
		var className = 'list__item list__item--track mouse-draggable mouse-selectable mouse-contextable';
		if (this.props.selected) className += ' list__item--selected';
		if (this.props.can_sort) className += ' list__item--can-sort';
		if (track.type !== undefined) className += ' list__item--'+track.type;
		if (track.playing) className += ' list__item--playing';
		if (this.state.hover) className += ' list__item--hover';

		let track_details = [];
		let track_actions = [];

		if (track.artists){
			track_details.push(
				<li className="details__item details__item--artists" key="artists">
					{track.artists ? <ArtistSentence artists={track.artists} /> : '-'}
				</li>
			)
		}

		if (track.album){

			if (track.album.uri){
				var album = <URILink type="album" uri={track.album.uri}>{track.album.name}</URILink>;
			} else {
				var album = <span>{track.album.name}</span>;
			}

			track_details.push(
				<li className="details__item details__item--album" key="album">
					{album}
				</li>
			)
		}

		if (this.props.context == 'history'){
			var track_middle_column = (
				<div className="list__item__column__item list__item__column__item--played_at">
					{track.played_at ? <span><Dater type="ago" data={track.played_at} /> ago</span> : '-'}
				</div>
			)

		} else if (this.props.context == 'queue'){
			if (track.added_from && track.added_by){
				var type = (track.added_from ? helpers.uriType(track.added_from) : null);

				switch (type){
					case "discover":
						var link = <URILink type="recommendations" uri={helpers.getFromUri('seeds',track.added_from)}>discover</URILink>
						break;

					case "browse":
						var link = <URILink type="browse" uri={track.added_from.replace("iris:browse:","")}>browse</URILink>
						break;

					case "search":
						var link = <URILink type="search" uri={track.added_from.replace("iris:","")}>search</URILink>
						break;

					default:
						var link = <URILink type={type} uri={track.added_from}>{type}</URILink>;
				}

				var track_middle_column = (
					<div className="list__item__column__item list__item__column__item--added">
						<span className="by">{track.added_by} </span>
						<span className="from">(from {link})</span>
					</div>
				);

			} else if (track.added_by){
				var track_middle_column = (
					<div className="list__item__column__item list__item__column__item--added">
						<span className="by">{track.added_by}</span>
					</div>
				);
			}
		}

		/*
		if (this.props.show_source_icon){
			track_details.push(
				<li className="list__item__details__item list__item__details__item--source" key="source">
					<Icon type="fontawesome" name={helpers.sourceIcon(track.uri)} fixedWidth />
				</li>
			)
		}*/

		// If we're touchable, and can sort this tracklist
		var drag_zone = null;
		if (helpers.isTouchDevice() && this.props.can_sort){
			className += " has-touch-drag-zone"

			drag_zone = (
				<span 
					className="list__item__column__item list__item__column__item--drag-zone drag-zone touch-draggable mouse-draggable"
					key="drag-zone">
						<Icon name="drag_indicator" />
				</span>
			);
		}

		if (track_middle_column){
			className += " list__item--has-middle-column";
		}

		return (
			<ErrorBoundary>
				<div 
					className={className}
					onMouseEnter={e => this.handleMouseEnter(e)}
					onMouseLeave={e => this.handleMouseLeave(e)}
					onMouseDown={e => this.handleMouseDown(e)}
					onMouseUp={e => this.handleMouseUp(e)}
					onMouseMove={e => this.handleMouseMove(e)}
					onDoubleClick={e => this.handleDoubleClick(e)}
					onContextMenu={e => this.props.handleContextMenu(e)}
					onTouchStart={e => this.handleTouchStart(e)}
					onTouchEnd={e => this.handleTouchEnd(e)}>
						<div className="list__item__column list__item__column--right">
							{drag_zone}
							<span className="list__item__column__item list__item__column__item--duration">
								{track.duration ? <Dater type="length" data={track.duration} /> : '-'}
							</span>
							<ContextMenuTrigger className="list__item__column__item--context-menu-trigger subtle" onTrigger={e => this.props.handleContextMenu(e)} />
						</div>
						<div className="list__item__column list__item__column--name">
							<div className="list__item__column__item--name">
								{track.name ? track.name : <span className="mid_grey-text">{track.uri}</span>}
								{track.explicit ? <span className="flag dark">EXPLICIT</span> : null}
								{track.playing ? <Icon className={"js--"+this.props.play_state} name="playing" type="css"></Icon> : null}
							</div>
							<ul className="list__item__column__item--details">
								{track_details}
							</ul>
						</div>
						{track_middle_column ? <div className="list__item__column list__item__column--middle">{track_middle_column}</div> : null}
				</div>
			</ErrorBoundary>
		);
	}
}