
import React, { PropTypes } from 'react'
import GridItem from './GridItem'

export default class ArtistGrid extends React.Component{

	constructor(props) {
		super(props);
	}

	render(){
		if( this.props.artists ){
			var className = "grid artist-grid"
			if( this.props.className ) className += ' '+this.props.className
			return (
				<div className={className}>
					{
						this.props.artists.map(
							(artist, index) => {
								return <GridItem item={artist} key={index} link={global.baseURL+'artist/'+artist.uri} />
							}
						)
					}
				</div>
			);
		}
		return null;
	}
}

