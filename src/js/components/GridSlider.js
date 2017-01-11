
import React, { PropTypes } from 'react'
import GridItem from './GridItem'

export default class GridSlider extends React.Component{

	constructor(props) {
		super(props);
	}

	render(){
		if( this.props.tracks ){
			var className = "grid-slider"
			if( this.props.className ) className += ' '+this.props.className
			return (
				<div className={className}>
					<div className="grid artist-grid liner">
						{
							this.props.tracks.map(
								(track, index) => {
									var item = Object.assign({}, track.album, { artists: track.artists })
									return <GridItem item={item} key={index} link={global.baseURL+'album/'+track.album.uri} />
								}
							)
						}
					</div>
				</div>
			);
		}
		return null;
	}
}

