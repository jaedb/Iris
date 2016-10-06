
import React, { PropTypes } from 'react'
import Track from './Track'

export default class TrackList extends React.Component{

	constructor(props) {
		super(props);

		this.state = {
			tracks: this.props.tracks
		}
	}

	toggleSelected( index ){
		var tracks = this.state.tracks;
		tracks[index].selected = !tracks[index].selected;
		this.setState({ tracks: tracks });
	}

	playTrack( index ){
		var tracks = this.state.tracks;
		console.log('playTrack', tracks[index].tlid)
	}

	componentWillReceiveProps( nextProps ){
		this.setState({ tracks: nextProps.tracks });
	}

	render(){
		let self = this;
		if( this.state.tracks ){
			return (
				<ul>
					{
						this.state.tracks.map(
							(track, index) => {

								// flatten nested track objects (as in the case of TlTracks)
								if( typeof(track.track) !== 'undefined' ){
									for( var property in track.track ){
										if( track.track.hasOwnProperty(property) ){
											track[property] = track.track[property]
										}
									}
								}
								return <Track
										key={index+'_'+track.uri} 
										track={track} 
										playTrack={() => self.playTrack(index)}
										toggleSelected={() => self.toggleSelected(index)} />
							}
						)
					}
				</ul>
			);
		}
		return null;
	}
}