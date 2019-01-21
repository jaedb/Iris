
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { createStore, bindActionCreators } from 'redux'


import * as helpers from '../helpers'
import * as uiActions from '../services/ui/actions'
import GridItem from './GridItem'

class PlaylistGrid extends React.Component{

	constructor(props){
		super(props);
	}

	handleContextMenu(e,item){
		e.preventDefault()
		var data = {
			e: e,
			context: 'playlist',
			uris: [item.uri],
			items: [item],
			tracklist_uri: item.uri
		}
		this.props.uiActions.showContextMenu(data)
	}

	render(){
		if (!this.props.playlists ) return null

		var className = "grid grid--playlists";
		if (this.props.className) className += ' '+this.props.className;
		if (this.props.single_row) className += ' grid--single-row';
		if (this.props.mini) className += ' grid--mini';

		return (
			<div className={className}>
				{
					this.props.playlists.map(playlist => {							
						return (
							<GridItem
								key={playlist.uri}
								type="playlist"
								item={playlist}
								show_source_icon={this.props.show_source_icon}
								onClick={e => {this.props.history.push(global.baseURL+'playlist/'+encodeURIComponent(playlist.uri))}}
								onContextMenu={e => this.handleContextMenu(e,playlist)}
							/>
						)}
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

