
import React, { PropTypes } from 'react'
import FontAwesome from 'react-fontawesome'
import { Link } from 'react-router'

export default class ArtistSentence extends React.Component{

	constructor(props) {
		super(props);
	}

	render(){
		if( !this.props.artists ) return <span>-</span>

		return (
			<span className={ this.props.className ? this.props.className+" artist-sentence" : "artist-sentence" }>
				{
					this.props.artists.map( (artist, index) => {
						if (!artist) return <span>-</span>
						var separator = null;
						if( index == this.props.artists.length - 2 ){
							separator = ' and ';
						}else if( index < this.props.artists.length - 2 ){
							separator = ', ';
						}
						if (!artist.name){							
							var content = <span>-</span>
						} else if (!artist.uri || this.props.nolinks){
							var content = <span>{ artist.name }</span>
						}else{
							var content = <Link className="artist" to={global.baseURL+'artist/'+artist.uri}>{ artist.name }</Link>
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