
import React, { PropTypes } from 'react'
import GridItem from './GridItem'

export default class ArtistGrid extends React.Component{

	constructor(props) {
		super(props);
	}

	render(){
		if( this.props.artists ){
			return (
				<ul className="grid artist-grid">
					{
						this.props.artists.map(
							(artist, index) => {
								var link = artist.uri;
								return <GridItem item={artist} key={index} link={'/artist/'+link} />
							}
						)
					}
				</ul>
			);
		}
		return null;
	}
}

