
import React, { PropTypes } from 'react'
import Track from './Track'

export default class TrackList extends React.Component{

	constructor(props) {
		super(props);
	}

	render(){

		if( this.props.tracks ){
			return (
				<ul>
					{
						this.props.tracks.map( track => {
								if( typeof(track.track) !== 'undefined' ){
									track.track.tlid = track.tlid;
									track = track.track;
								}
								return <Track key={track.uri} track={track} />
							}
						)
					}
				</ul>
			);
		}
		return null;
	}
}