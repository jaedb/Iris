
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { createStore, bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'

import Thumbnail from './Thumbnail'
import URILink from './URILink'

import * as helpers from '../helpers'
import * as uiActions from '../services/ui/actions'

class ArtistGrid extends React.Component{

	constructor(props){
		super(props);
	}

	handleContextMenu(e,item){
		e.preventDefault()
		var data = {
			e: e,
			context: 'artist',
			uris: [item.uri],
			items: [item]
		}
		this.props.uiActions.showContextMenu(data)
	}

	render(){
		if (this.props.artists){
			var className = "grid artist-grid"
			if (this.props.className) className += ' '+this.props.className
			if (this.props.single_row) className += ' single-row'
				
			return (
				<div className={className}>
					{
						this.props.artists.map(
							(artist, index) => {
								return (
									<URILink 
										className="grid-item"
										type="artist"
										uri={artist.uri}
										key={index} 
										onContextMenu={e => this.handleContextMenu(e,artist)}>
											<Thumbnail size="medium" images={artist.images} />
											<div className="name">
												{artist.name}
											</div>
											<div className="secondary">
												{this.props.show_source_icon ? <FontAwesome name={helpers.sourceIcon(artist.uri)} className="source" /> : null}
												{artist.followers ? artist.followers.total.toLocaleString()+' followers' : null}
												{artist.albums_uris && !artist.followers ? artist.albums_uris.length+' albums' : null}
											</div>
									</URILink>
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

export default connect(mapStateToProps, mapDispatchToProps)(ArtistGrid)

