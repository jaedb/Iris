
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

								// handle nested album objects (as in Album Library)
								if( album.album ){
									var flatAlbum = album.album;
									flatAlbum.added_at = album.added_at;
									album = flatAlbum;
								}

								return <GridItem item={album} key={index} link={'/album/'+album.uri} />
							}
						)
					}
				</div>
			);
		}
		return null;
	}
}

