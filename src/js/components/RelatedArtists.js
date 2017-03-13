
import React, { PropTypes } from 'react'
import { Link } from 'react-router'
import Thumbnail from './Thumbnail'

export default class RelatedArtists extends React.Component{

	constructor(props) {
		super(props)
	}

	render(){
		if( !this.props.artists ) return null

		return (
			<div className="list related-artist-list">
				{
					this.props.artists.map( (artist, index) => {
						if( artist.uri ){
							return (
								<Link to={global.baseURL+'artist/'+ artist.uri} key={artist.uri} className="artist">
									<Thumbnail circle={true} size="small" images={artist.images} />
									<span className="name">{ artist.name }</span>
								</Link>
							)
						}else{
							return (
								<span key={artist.uri} className="artist">
									<Thumbnail circle={true} size="small" images={artist.images} />
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