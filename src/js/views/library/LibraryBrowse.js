
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link } from 'react-router'
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

	// on render
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
				uri = props.params.uri.replace('|','?')
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
				folders.push(Object.assign(
					{},
					directory[i],
					{
						uri: directory[i].uri.replace('?','|')
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
	}

	render(){
		if (typeof(this.props.params.uri) !== 'undefined' && this.props.params.uri){
			
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
						<Header icon="music" title="Browse" options={options} uiActions={this.props.uiActions} />
						<div className="body-loader">
							<div className="loader"></div>
						</div>
					</div>
				)
			}

			var items = this.arrange_directory( this.props.directory )

			return (
				<div className="view library-local-view">
					<Header icon="music" title={this.props.params.uri} options={options} uiActions={this.props.uiActions} />
					<section className="content-wrapper">
						<List
							columns={[{ name: 'name', width: '100'}]} 
							rows={items.folders} 
							className="library-local-directory-list"
							link_prefix={global.baseURL+"library/browse/"} />
						<TrackList 
							tracks={items.tracks} 
							className="library-local-track-list" 
							noheader />
					</section>
				</div>
			);

		} else {				

			var grid_items = [
				{
					name: 'Local artists',
					link: global.baseURL+'library/local/artists',
					icons: ['assets/backgrounds/category-artists.jpg']
				},
				{
					name: 'Local albums',
					link: global.baseURL+'library/local/albums',
					icons: ['assets/backgrounds/category-albums.jpg']
				}
			]

			if (this.props.directory){
				for (var i = 0; i < this.props.directory.length; i++){
					var directory = this.props.directory[i]

					switch (directory.name){
						case 'Files':
							directory.icons = ['assets/backgrounds/category-folders.jpg']
							break

						default:
							directory.icons = ['assets/backgrounds/category-albums.jpg']
					}

					grid_items.push({
						name: directory.name,
						link: global.baseURL+'library/browse/'+directory.uri,
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
										return <GridItem item={item} key={index} link={item.link} />
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