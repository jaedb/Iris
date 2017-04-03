
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { createStore, bindActionCreators } from 'redux'
import { Link } from 'react-router'
import FontAwesome from 'react-fontawesome'

import Thumbnail from './Thumbnail'

import * as helpers from '../helpers'
import * as uiActions from '../services/ui/actions'

class PlaylistGrid extends React.Component{

	constructor(props) {
		super(props);
	}

	handleContextMenu(e,item){
		e.preventDefault()
		var data = {
			e: e,
			context: 'playlist',
			uris: [item.uri],
			items: [item]
		}
		this.props.uiActions.showContextMenu(data)
	}

	render(){
		if( !this.props.playlists ) return null

		var className = "grid playlist-grid"
		if( this.props.className ) className += ' '+this.props.className
		return (
			<div className={className}>
				{
					this.props.playlists.map(
						(playlist, index) => {							
							return (
								<Link 
									className="grid-item"
									to={global.baseURL+'playlist/'+playlist.uri}
									key={index} 
									onContextMenu={e => this.handleContextMenu(e,playlist)}>
										<Thumbnail size="medium" images={playlist.images} />
										<div className="name">
											{playlist.name}
											{this.props.show_source_icon ? <FontAwesome name={helpers.sourceIcon(playlist.uri)} className="source" fixedWidth /> : null}
										</div>
										<div className="secondary">
											{playlist.tracks_total ? playlist.tracks_total+' tracks' : <span>0 tracks</span>}
										</div>
								</Link>
							)
						}
					)
				}
			</div>
		);
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

export default connect(mapStateToProps, mapDispatchToProps)(PlaylistGrid)

