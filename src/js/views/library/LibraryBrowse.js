
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link, hashHistory } from 'react-router'
import FontAwesome from 'react-fontawesome'

import Header from '../../components/Header'
import List from '../../components/List'
import TrackList from '../../components/TrackList'
import GridItem from '../../components/GridItem'
import Icon from '../../components/Icon'

import * as helpers from '../../helpers'
import * as uiActions from '../../services/ui/actions'
import * as mopidyActions from '../../services/mopidy/actions'
import * as spotifyActions from '../../services/spotify/actions'

class LibraryBrowse extends React.Component{

	constructor(props){
		super(props);
	}

	componentDidMount(){
		this.loadDirectory()
	}

	componentWillReceiveProps(nextProps){

		// mopidy goes online
		if (!this.props.mopidy_connected && nextProps.mopidy_connected){
			this.loadDirectory(nextProps );
		}

		// our uri changes
		if (nextProps.params.uri != this.props.params.uri){
			this.loadDirectory(nextProps );
		}
	}

	loadDirectory(props = this.props){
		if (props.mopidy_connected){
			var uri = null
			if (props.params.uri !== undefined){
				uri = props.params.uri;
			}
			this.props.mopidyActions.getDirectory(uri);
		}
	}

	playAll(e){
		var tracks = this.arrangeDirectory().tracks;
		var tracks_uris = helpers.arrayOf('uri',tracks);

		this.props.mopidyActions.playURIs(tracks_uris, "iris:browse:"+this.props.params.uri);
		this.props.uiActions.hideContextMenu();
	}

	goBack(e){
		window.history.back();
		this.props.uiActions.hideContextMenu();
	}

	arrangeDirectory(directory = this.props.directory){
		var folders = []
		var tracks = []

		for (var i = 0; i < directory.length; i++){
			if (directory[i].type && directory[i].type == 'track'){
				tracks.push(directory[i] )
			} else {
				folders.push(Object.assign(
					{},
					directory[i],
					{
						uri: directory[i].uri
					}
				))
			}
		}

		return {
			folders: folders,
			tracks: tracks
		}
	}

	renderDirectory(){
		var title = 'Browse';
		var uri_exploded = this.props.params.uri.split(':');
		if (uri_exploded.length > 0){
			title = uri_exploded[0];
			title = title.charAt(0).toUpperCase() + title.slice(1);
		}

		if (!this.props.directory || helpers.isLoading(this.props.load_queue,['mopidy_browse'])){
			return (
				<div className="view library-local-view">
					<Header icon="music" title={title} uiActions={this.props.uiActions} />
					<div className="body-loader loading">
						<div className="loader"></div>
					</div>
				</div>
			)
		}

		var items = this.arrangeDirectory(this.props.directory);

		var options = (
			<span>
				<button className="no-hover" onClick={e => this.playAll(e)}>
					<FontAwesome name="play" />&nbsp;
					Play all
				</button>
				<button className="no-hover" onClick={e => this.goBack(e)}>
					<FontAwesome name="reply" />&nbsp;
					Back
				</button>
			</span>
		);

		return (
			<div className="view library-local-view">
				<Header options={options} uiActions={this.props.uiActions}>
					<Icon name="folder" type="material" />
					{title}
				</Header>
				<section className="content-wrapper">
					<List
						nocontext
						columns={[{ name: 'name', width: '100'}]} 
						rows={items.folders} 
						className="library-local-directory-list"
						link_prefix={global.baseURL+'library/browse/'}
					/>
					<TrackList 
						tracks={items.tracks}
						uri={"iris:browse:"+this.props.params.uri}
						className="library-local-track-list" 
						noheader />
				</section>
			</div>
		);
	}

	renderIndex(){
		var grid_items = []
		if (this.props.directory){
			for (var i = 0; i < this.props.directory.length; i++){
				var directory = this.props.directory[i]

				switch (directory.name){
					case 'Dirble':
						directory.icons = ['assets/backgrounds/browse-dirble.jpg']
						break

					case 'Files':
						directory.icons = ['assets/backgrounds/browse-folders.jpg']
						break

					case 'Local media':
						directory.icons = ['assets/backgrounds/browse-folders.jpg']
						break

					case 'Spotify':
					case 'Spotify Browse':
						directory.icons = ['assets/backgrounds/browse-spotify.jpg']
						break

					case 'Spotify Tunigo':
					case 'Tunigo':
						directory.icons = ['assets/backgrounds/browse-tunigo.jpg']
						break

					case 'TuneIn':
						directory.icons = ['assets/backgrounds/browse-tunein.jpg']
						break

					case 'SoundCloud':
						directory.icons = ['assets/backgrounds/browse-soundcloud.jpg']
						break

					case 'iTunes Store: Podcasts':
						directory.icons = ['assets/backgrounds/browse-itunes.jpg']
						break

					case 'Soma FM':
						directory.icons = ['assets/backgrounds/browse-somafm.jpg']
						break

					default:
						directory.icons = ['assets/backgrounds/browse-default.jpg']
				}

				grid_items.push({
					name: directory.name,
					link: global.baseURL+'library/browse/'+encodeURIComponent(directory.uri),
					icons: directory.icons
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
						{
							grid_items.map(
								(item, index) => {
									return (
										<GridItem 
											item={item} 
											key={index} 
											onClick={e => hashHistory.push(item.link)}
										/>
									)
								}
							)
						}
					</div>
				</section>
			</div>
		);
	}

	render(){
		if (this.props.params.uri !== undefined && this.props.params.uri){
			return this.renderDirectory();
		} else {
			return this.renderIndex();
		}
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		load_queue: state.ui.load_queue,
		mopidy_connected: state.mopidy.connected,
		directory: state.mopidy.directory
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(LibraryBrowse)