
import React, { PropTypes } from 'react'
import GridItem from './GridItem'

export default class PlaylistGrid extends React.Component{

	constructor(props) {
		super(props);
	}

	render(){
		if( this.props.playlists ){
			var className = "grid playlist-grid"
			if( this.props.className ) className += ' '+this.props.className
			return (
				<div className={className}>
					{
						this.props.playlists.map(
							(playlist, index) => {
								return <GridItem item={playlist} key={index} link={'/playlist/'+playlist.uri} />
							}
						)
					}
				</div>
			);
		}
		return null;
	}
}

