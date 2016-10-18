
import React, { PropTypes } from 'react'
import AlbumGridItem from './AlbumGridItem'

export default class AlbumGrid extends React.Component{

	constructor(props) {
		super(props);
	}

	render(){
		if( this.props.items ){
			return (
				<ul className="grid album-grid">
					{
						this.props.items.items.map(
							(album, index) => {
								return <AlbumGridItem item={album} key={index} />
							}
						)
					}
				</ul>
			);
		}
		return null;
	}
}

