
import React, { PropTypes } from 'react'
import FontAwesome from 'react-fontawesome'
import { Link } from 'react-router'

export default class ArtistSentence extends React.Component{

	constructor(props) {
		super(props);
	}

	render(){
		return (
			<span className="artist-sentence">
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
							<span key={artist.uri}>
								<Link className="artist" to={ link }>{ artist.name }</Link>{ separator }
							</span>
						);
					})
				}
			</span>
		);
	}
}