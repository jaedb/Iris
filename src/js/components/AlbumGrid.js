
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { createStore, bindActionCreators } from 'redux'
import { hashHistory } from 'react-router'
import FontAwesome from 'react-fontawesome'

import * as helpers from '../helpers'
import * as uiActions from '../services/ui/actions'
import GridItem from './GridItem'

class AlbumGrid extends React.Component{

	constructor(props) {
		super(props)
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
						this.props.albums.map(album => {
							return (
								<GridItem
									key={album.uri}
									type="album"
									item={album}
									onClick={e => {hashHistory.push(global.baseURL+'album/'+album.uri)}}
									onLoad={() => this.handleLoad(album.uri)}
									onContextMenu={e => this.handleContextMenu(e,album)}
								/>
							)}
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

