
import React, { PropTypes } from 'react'
import Thumbnail from './Thumbnail'
import URILink from './URILink'

export default class RelatedArtists extends React.Component{

	constructor(props){
		super(props)
	}

	render(){
		if (!this.props.artists ) return null

		return (
			<div className="list related-artist-list">
				{
					this.props.artists.map((artist, index) => {

						var images = artist.images;
						if (Array.isArray(images)){
							images = images[0];
						}

						if (artist.uri){
							return (
								<URILink type="artist" uri={artist.uri} key={artist.uri} className="artist">
									<Thumbnail circle={true} size="small" images={images} />
									<span className="name">{ artist.name }</span>
								</URILink>
							)
						} else {
							return (
								<span key={artist.uri} className="artist">
									<Thumbnail circle={true} size="small" images={images} />
									<span className="name">{ artist.name }</span>
								</span>
							)
						}
					})
				}
			</div>
		);
	}
}