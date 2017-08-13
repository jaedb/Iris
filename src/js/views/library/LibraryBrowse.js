
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link, hashHistory } from 'react-router'
import FontAwesome from 'react-fontawesome'

import Header from '../../components/Header'
import List from '../../components/List'
import TrackList from '../../components/TrackList'
import GridItem from '../../components/GridItem'

import * as helpers from '../../helpers'
import * as mopidyActions from '../../services/mopidy/actions'
import * as spotifyActions from '../../services/spotify/actions'

class LibraryBrowse extends React.Component{

	constructor(props) {
		super(props);
	}

	componentDidMount(){
		this.loadDirectory()
	}

	componentWillReceiveProps( nextProps ){

		// mopidy goes online
		if (!this.props.mopidy_connected && nextProps.mopidy_connected ){
			this.loadDirectory( nextProps );
		}

		// our uri changes
		if (nextProps.params.uri != this.props.params.uri){
			this.loadDirectory( nextProps );
		}
	}

	loadDirectory( props = this.props ){
		if (props.mopidy_connected){
			var uri = null
			if (typeof(props.params.uri) !== 'undefined'){
				uri = decodeURIComponent(props.params.uri)
			}

			this.props.mopidyActions.getDirectory(uri)
		}
	}

	arrange_directory( directory ){
		var folders = []
		var tracks = []

		for (var i = 0; i < directory.length; i++) {
			if (directory[i].type && directory[i].type == 'track') {
				tracks.push( directory[i] )
			} else {
				var uri = directory[i].uri

				// If we've navigated to a handled asset type, use our standard views
				switch (helpers.uriType(uri)){
					case 'album':
						uri = global.baseURL+'album/'+uri
						break

					case 'artist':
						uri = global.baseURL+'artist/'+uri
						break

					case 'playlist':
						uri = global.baseURL+'playlist/'+uri
						break

					default:
						uri = global.baseURL+"library/browse/"+encodeURIComponent(uri)
				}

				folders.push(Object.assign(
					{},
					directory[i],
					{
						uri: uri
					}
				))
			}
		}

		return {
			folders: folders,
			tracks: tracks
		}
	}

	render(){
		if (typeof(this.props.params.uri) !== 'undefined' && this.props.params.uri){
			
			var title = 'Browse'
			var uri_exploded = this.props.params.uri.split(':')
			if (uri_exploded.length > 0){
				title = uri_exploded[0]
				title = title.charAt(0).toUpperCase() + title.slice(1)
			}

			var options = null
			if (this.props.params.uri != 'local:directory' ){
				options = (
					<button onClick={ () => window.history.back() }>
						<FontAwesome name="reply" />&nbsp;
						Back
					</button>
				)
			}

			if (!this.props.directory || helpers.isLoading(this.props.load_queue,['mopidy_browse'])){
				return (
					<div className="view library-local-view">
						<Header icon="music" title={title} options={options} uiActions={this.props.uiActions} />
						<div className="body-loader loading">
							<div className="loader"></div>
						</div>
					</div>
				)
			}

			var items = this.arrange_directory( this.props.directory )

			return (
				<div className="view library-local-view">
					<Header icon="music" title={title} options={options} uiActions={this.props.uiActions} />
					<section className="content-wrapper">
						<List
							columns={[{ name: 'name', width: '100'}]} 
							rows={items.folders} 
							className="library-local-directory-list" />
						<TrackList 
							tracks={items.tracks} 
							className="library-local-track-list" 
							noheader />
					</section>
				</div>
			);

		} else {				

			var grid_items = []
			if (this.props.directory){
				for (var i = 0; i < this.props.directory.length; i++){
					var directory = this.props.directory[i]

					switch (directory.name){
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
					<Header icon="folder" title="Browse" />
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
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(LibraryBrowse)