
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link } from 'react-router'
import FontAwesome from 'react-fontawesome'

import Header from '../../components/Header'
import List from '../../components/List'
import TrackList from '../../components/TrackList'

import * as helpers from '../../helpers'
import * as mopidyActions from '../../services/mopidy/actions'
import * as spotifyActions from '../../services/spotify/actions'

class LibraryLocalDirectory extends React.Component{

	constructor(props) {
		super(props);
	}

	// on render
	componentDidMount(){
		this.loadDirectory()
	}

	componentWillReceiveProps( nextProps ){

		// mopidy goes online
		if( !this.props.mopidy_connected && nextProps.mopidy_connected ){
			this.loadDirectory( nextProps );
		}

		// our uri changes
		if( nextProps.params.uri != this.props.params.uri ){
			this.loadDirectory( nextProps );
		}
	}

	loadDirectory( props = this.props ){
		if( props.mopidy_connected ){
			this.props.mopidyActions.getDirectory( props.params.uri.replace('|','?') );
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
					<Header icon="music" title="Local files" options={options} uiActions={this.props.uiActions} />
					<div className="body-loader">
						<div className="loader"></div>
					</div>
				</div>
			)
		}

		var items = this.arrange_directory( this.props.directory )

		return (
			<div className="view library-local-view">
				<Header icon="music" title="Local files" options={options} uiActions={this.props.uiActions} />
				<section className="list-wrapper">
					<List
						columns={[{ name: 'name', width: '100'}]} 
						rows={items.folders} 
						className="library-local-directory-list"
						link_prefix={global.baseURL+"library/local/directory/"} />
					<TrackList 
						tracks={items.tracks} 
						className="library-local-track-list" 
						noheader />
				</section>
			</div>
		);
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

export default connect(mapStateToProps, mapDispatchToProps)(LibraryLocalDirectory)