
import React, { PropTypes } from 'react';
import { connect } from 'react-redux';
import { bindActionCreators } from 'redux';
import { Route, Switch } from 'react-router-dom';

import Link from '../../components/Link';

import Header from '../../components/Header';
import List from '../../components/List';
import TrackList from '../../components/TrackList';
import GridItem from '../../components/GridItem';
import DropdownField from '../../components/Fields/DropdownField';
import Icon from '../../components/Icon';
import URILink from '../../components/URILink';
import ErrorBoundary from '../../components/ErrorBoundary';

import * as helpers from '../../helpers';
import * as uiActions from '../../services/ui/actions';
import * as mopidyActions from '../../services/mopidy/actions';

class LibraryBrowse extends React.Component{

	constructor(props){
		super(props);
	}

	componentDidMount(){
		this.loadDirectory()
	}

	componentWillReceiveProps(nextProps){
		this.props.uiActions.setWindowTitle("Browse");

		// mopidy goes online
		if (!this.props.mopidy_connected && nextProps.mopidy_connected){
			this.loadDirectory(nextProps);
		}
	}

	loadDirectory(props = this.props){
		if (props.mopidy_connected){
			var uri = null
			if (props.uri !== undefined){
				uri = props.uri;
			}
			this.props.mopidyActions.getDirectory(uri);
		}
	}

	render(){
		var grid_items = []
		if (this.props.directory){
			for (var subdirectory of this.props.directory.subdirectories){

				switch (subdirectory.name){
					case 'Dirble':
						subdirectory.icons = ['assets/backgrounds/browse-dirble.jpg']
						break

					case 'Files':
						subdirectory.icons = ['assets/backgrounds/browse-folders.jpg']
						break

					case 'Local media':
						subdirectory.icons = ['assets/backgrounds/browse-folders.jpg']
						break

					case 'Spotify':
					case 'Spotify Browse':
						subdirectory.icons = ['assets/backgrounds/browse-spotify.jpg']
						break

					case 'Spotify Tunigo':
					case 'Tunigo':
						subdirectory.icons = ['assets/backgrounds/browse-tunigo.jpg']
						break

					case 'TuneIn':
						subdirectory.icons = ['assets/backgrounds/browse-tunein.jpg']
						break

					case 'SoundCloud':
						subdirectory.icons = ['assets/backgrounds/browse-soundcloud.jpg']
						break

					case 'iTunes Store: Podcasts':
						subdirectory.icons = ['assets/backgrounds/browse-itunes.jpg']
						break

					case 'Soma FM':
						subdirectory.icons = ['assets/backgrounds/browse-somafm.jpg']
						break

					default:
						subdirectory.icons = ['assets/backgrounds/browse-default.jpg']
				}

				grid_items.push({
					name: subdirectory.name,
					link: global.baseURL+'library/browse/'+encodeURIComponent(subdirectory.uri),
					icons: helpers.formatImages(subdirectory.icons)
				})
			}
		}

		return (
			<div className="view library-local-view">
				<Header>				
					<Icon name="folder" type="material" />
					Browse
				</Header>
				<section className="content-wrapper">
					<div className="grid category-grid">
						<ErrorBoundary>
							{
								grid_items.map(
									(item, index) => {
										return (
											<GridItem 
												item={item} 
												key={index}
												type="category"
											/>
										)
									}
								)
							}
						</ErrorBoundary>
					</div>
				</section>
			</div>
		);
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		load_queue: state.ui.load_queue,
		mopidy_connected: state.mopidy.connected,
		directory: state.mopidy.directory,
		view: state.ui.library_directory_view
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(LibraryBrowse)