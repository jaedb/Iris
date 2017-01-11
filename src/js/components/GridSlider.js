
import React, { PropTypes } from 'react'
import FontAwesome from 'react-fontawesome'
import GridItem from './GridItem'

export default class GridSlider extends React.Component{

	constructor(props) {
		super(props)

		this._pagelimit = 3

		this.state = {
			page: 0
		}
	}

	next(){
		if (this.state.page >= this._pagelimit) return false
		this.setState({ page: this.state.page + 1 })
	}

	previous(){
		if (this.state.page <= 0) return false
		this.setState({ page: this.state.page - 1 })
	}

	render(){
		if( this.props.tracks ){

			var className = "grid-slider-wrapper"
			if( this.props.className ) className += ' '+this.props.className

			var style = {
				left: '-'+(this.state.page * 100)+'%'
			}

			return (
				<div className={className}>
					{ this.props.title ? this.props.title : null }
					<div className="controls">
						<FontAwesome name="chevron-left" disabled={this.state.page <= 0} onClick={ () => this.previous() } />
						<FontAwesome name="chevron-right" disabled={this.state.page >= this._pagelimit} onClick={ () => this.next() } />
					</div>
					<div className="grid-slider">
						<div className="grid artist-grid liner" style={style}>
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
				</div>
			);
		}
		return null;
	}
}

