
import React, { PropTypes } from 'react'

export default class Tracklist extends React.Component{

	constructor(props) {
		super(props);
	}

	flattenTracks(){
		var tracks = [];
		if( this.props.tracks ){
			var originalTracks = this.props.tracks;

			for( var i = 0; i < originalTracks.length; i++ ){
				var track = originalTracks[i];
				if( typeof(track.track) !== 'undefined' ){
					track.track.tlid = track.tlid;
					track = track.track;
				}
				tracks.push( track );
			}
		}
		return tracks;
	}

	render(){

		if( this.flattenTracks ){
			return (
				<ul>
					{
						this.flattenTracks().map( track => 
							<li key={track.uri}>{ track.name }</li>
						)
					}
				</ul>
			);
		}
		return null;
	}
}