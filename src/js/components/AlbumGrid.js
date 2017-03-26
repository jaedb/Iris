
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { createStore, bindActionCreators } from 'redux'
import { hashHistory } from 'react-router'
import FontAwesome from 'react-fontawesome'

import Thumbnail from './Thumbnail'
import ArtistSentence from './ArtistSentence'

import * as helpers from '../helpers'
import * as uiActions from '../services/ui/actions'

class AlbumGrid extends React.Component{

	constructor(props) {
		super(props);
	}

	handleClick(e,link){
		if( e.target.tagName.toLowerCase() !== 'a' ){
			hashHistory.push(link)
		}
	}

	handleContextMenu(e,item){
		e.preventDefault()
		var data = { 
			e: e,
			context: 'album',
			uris: [item.uri],
			items: [item]
		}
		this.props.uiActions.showContextMenu(data)
	}

	render(){
		if( this.props.albums ){
			var className = "grid album-grid"
			if( this.props.className ) className += ' '+this.props.className
			return (
				<div className={className}>
					{
						this.props.albums.map(
							(album, index) => {
								return (
									<div className="grid-item" 
										key={index} 
										onClick={ (e) => this.handleClick(e,global.baseURL+'album/'+album.uri) }
										onContextMenu={e => this.handleContextMenu(e,album)}>
											<Thumbnail size="medium" images={album.images} />
											<div className="name">
												{album.name}
												{this.props.show_source_icon ? <FontAwesome name={helpers.sourceIcon(album.uri)} className="source" fixedWidth /> : null}
											</div>
											<div className="secondary">
												{album.artists ? <ArtistSentence artists={album.artists} /> : <span>-</span>}
											</div>
									</div>
								)
							}
						)
					}
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

export default connect(mapStateToProps, mapDispatchToProps)(AlbumGrid)

