
import React, { PropTypes } from 'react'
import FontAwesome from 'react-fontawesome'
import { Link } from 'react-router'

export default class ArtistList extends React.Component{

	constructor(props) {
		super(props);
	}

	render(){
		return (
			<span className="artist-list">
				{
					this.props.artists.map( (artist, index) => {
						var separator = null;
						if( index == this.props.artists.length - 2 ){
							separator = ' and ';
						}else if( index < this.props.artists.length - 2 ){
							separator = ', ';
						}
						var link = '/artist/' + artist.uri;
						return (
							<span className="artist" key={artist.uri}>
								<Link to={ link }>{ artist.name }</Link>{ separator }
							</span>
						);
					})
				}
			</span>
		);
	}
}