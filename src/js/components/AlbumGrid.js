
import React, { PropTypes } from 'react'
import GridItem from './GridItem'

export default class AlbumGrid extends React.Component{

	constructor(props) {
		super(props);
	}

	render(){
		if( this.props.albums ){
			var className = "grid album-grid"
			if( this.props.className ) className += ' '+this.props.className
			return (
				<div className={className}>
					{
						this.props.albums.map(
							(album, index) => {
								var link = album.uri;
								if( album.album ) link = album.album.uri;
								return <GridItem item={album} key={index} link={'/album/'+link} />
							}
						)
					}
				</div>
			);
		}
		return null;
	}
}

