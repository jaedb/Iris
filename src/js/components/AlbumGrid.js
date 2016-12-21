
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
								return <GridItem item={album} key={index} link={global.baseURL+'album/'+album.uri} />
							}
						)
					}
				</div>
			);
		}
		return null;
	}
}

