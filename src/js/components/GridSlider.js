
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { createStore, bindActionCreators } from 'redux'
import { Link, hashHistory } from 'react-router'
import FontAwesome from 'react-fontawesome'

import ArtistSentence from './ArtistSentence'
import Thumbnail from './Thumbnail'

import * as uiActions from '../services/ui/actions'

class GridSlider extends React.Component{

	constructor(props) {
		super(props)

		this._pagelimit = 3

		this.state = {
			page: 0
		}
	}

	handleClick(e,link){
		if( e.target.tagName.toLowerCase() !== 'a' ){
			hashHistory.push(link)
		}
	}

	handleContextMenu(e,item){
		e.preventDefault()
		var data = { 
			uris: [item.uri],
			item: item
		}
		this.props.uiActions.showContextMenu( e, data, 'album', 'click' )
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
										var album = Object.assign({}, track.album, { artists: track.artists })
										return (
											<div className="grid-item" 
												key={index} 
												onClick={ (e) => this.handleClick(e,global.baseURL+'album/'+album.uri) }
												onContextMenu={e => this.handleContextMenu(e,album)}>
													<Thumbnail size="medium" images={album.images} />
													<div className="name">{ album.name }</div>
													<div className="secondary">
														{ album.artists ? <ArtistSentence artists={album.artists} /> : <span>-</span> }
													</div>
											</div>
										)
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

const mapStateToProps = (state, ownProps) => {
	return {}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(GridSlider)

