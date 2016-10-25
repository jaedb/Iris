
import React, { PropTypes } from 'react'
import { Link } from 'react-router'
import Thumbnail from './Thumbnail'

export default class RelatedArtistList extends React.Component{

	constructor(props) {
		super(props);
	}

	render(){
		return (
			<span className="related-artist-list">
				{
					this.props.artists.map( (artist, index) => {
						var link = '/artist/' + artist.uri;
						return (
							<Link to={ link } key={artist.uri} className="artist">
								<Thumbnail circle={true} size="small" images={artist.images} />
								{ artist.name }
							</Link>
						);
					})
				}
			</span>
		);
	}
}