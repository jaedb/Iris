
import React, { PropTypes } from 'react'
import FontAwesome from 'react-fontawesome'
import URILink from './URILink'

export default class ArtistSentence extends React.Component{

	constructor(props){
		super(props);
	}

	render(){
		if (!this.props.artists){
			return <span>-</span>;
		}

		return (
			<span className={ this.props.className ? this.props.className+" artist-sentence" : "artist-sentence" }>
				{
					this.props.artists.map((artist, index) => {

						if (!artist){
							return <span>-</span>;
						}

						var separator = null;
						if (index == this.props.artists.length - 2){
							separator = ' and ';
						} else if (index < this.props.artists.length - 2){
							separator = ', ';
						}

						if (!artist.name){							
							var content = <span>-</span>
						} else if (!artist.uri || this.props.nolinks){
							var content = <span>{ artist.name }</span>
						} else {
							var content = <URILink className="artist" type="artist" uri={artist.uri}>{ artist.name }</URILink>
						}

						return (
							<span key={'index_'+artist.uri}>
								{ content }
								{ separator }
							</span>
						);
					})
				}
			</span>
		);
	}
}