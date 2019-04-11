
import React, { memo } from 'react'
import URILink from './URILink'

export default memo((props) => {

	if (!props.artists){
		return <span className={props.className ? props.className+" artist-sentence" : "artist-sentence" }>-</span>;
	}

	return (
		<span className={ props.className ? props.className+" artist-sentence" : "artist-sentence" }>
			{
				props.artists.map((artist, index) => {

					if (!artist){
						return <span>-</span>;
					}

					var separator = null;
					if (index == props.artists.length - 2){
						separator = ' and ';
					} else if (index < props.artists.length - 2){
						separator = ', ';
					}

					if (!artist.name){							
						var content = <span>-</span>
					} else if (!artist.uri || props.nolinks){
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
});