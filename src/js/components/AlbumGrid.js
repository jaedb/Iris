
import React from 'react';
import { connect } from 'react-redux';
import { createStore, bindActionCreators } from 'redux';

import * as helpers from '../helpers';
import * as uiActions from '../services/ui/actions';
import * as lastfmActions from '../services/lastfm/actions';

import GridItem from './GridItem';

class AlbumGrid extends React.Component{

	constructor(props){
		super(props)
	}

	handleContextMenu(e,item){
		e.preventDefault()
		var data = { 
			e: e,
			context: 'album',
			uris: [item.uri],
			items: [item],
			tracklist_uri: item.uri
		}
		this.props.uiActions.showContextMenu(data)
	}

	render(){
		if (this.props.albums){
			var className = "grid grid--albums";
			if (this.props.className) className += ' '+this.props.className;
			if (this.props.single_row) className += ' grid--single-row';
			if (this.props.mini) className += ' grid--mini';
				
			return (
				<div className={className}>
					{
						this.props.albums.map(item => {
							var album = helpers.collate(item, {artists: this.props.artists});
							return (
								<GridItem
									key={album.uri}
									type="album"
									item={album}
									lastfmActions={this.props.lastfmActions}
									show_source_icon={this.props.show_source_icon}
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
	return {
		artists: state.core.artists
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		lastfmActions: bindActionCreators(lastfmActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(AlbumGrid)

