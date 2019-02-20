
import React from 'react';
import { connect } from 'react-redux';
import { createStore, bindActionCreators } from 'redux';

import Thumbnail from './Thumbnail';
import GridItem from './GridItem';

import * as helpers from '../helpers';
import * as uiActions from '../services/ui/actions';
import * as lastfmActions from '../services/lastfm/actions';

class ArtistGrid extends React.Component{

	constructor(props){
		super(props);
	}

	handleContextMenu(e,item){
		e.preventDefault();
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
			var className = "grid grid--artists";
			if (this.props.className) className += ' '+this.props.className;
			if (this.props.single_row) className += ' grid--single-row';
			if (this.props.mini) className += ' grid--mini';
				
			return (
				<div className={className}>
					{
						this.props.artists.map(item => {
								var artist = helpers.collate(item, {albums: this.props.albums});
								return (
									<GridItem
										key={artist.uri}
										type="artist"
										item={artist}
										show_source_icon={this.props.show_source_icon}
										onClick={e => {this.props.history.push('/artist/'+encodeURIComponent(artist.uri))}}
										lastfmActions={this.props.lastfmActions}
										onContextMenu={e => this.handleContextMenu(e,artist)}
									/>
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
	return {
		albums: state.core.albums
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		lastfmActions: bindActionCreators(lastfmActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(ArtistGrid)

