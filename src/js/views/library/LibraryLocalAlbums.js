
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import Header from '../../components/Header'
import List from '../../components/List'

import * as mopidyActions from '../../services/mopidy/actions'
import * as spotifyActions from '../../services/spotify/actions'

class LibraryLocalAlbums extends React.Component{

	constructor(props) {
		super(props);
	}

	componentDidMount(){
		this.loadAlbums()
	}

	componentWillReceiveProps( nextProps ){
		if( !this.props.mopidy_connected && nextProps.mopidy_connected ){
			this.loadAlbums(nextProps);
		}
	}

	loadAlbums(props = this.props){
		if( props.mopidy_connected ){
			this.props.mopidyActions.getAlbums();
		}
	}

	render(){
		var albums = []
		if (this.props.albums && this.props.local_albums){
			for (var i = 0; i < this.props.local_albums.length; i++){
				var uri = this.props.local_albums[i]
				if (this.props.albums.hasOwnProperty(uri)){
					albums.push(this.props.albums[uri])
				}
			}
		}

		return (
			<div className="view library-local-view">
				<Header icon="music" title="Local albums" />
				<div>
					<List columns={[{ name: 'name', width: '100'}]} rows={albums} link_prefix={global.baseURL+"album/"} />
				</div>
			</div>
		);
	}
}


/**
 * Export our component
 *
 * We also integrate our global store, using connect()
 **/

const mapStateToProps = (state, ownProps) => {
	return {
		mopidy_connected: state.mopidy.connected,
		local_albums: state.ui.local_albums,
		albums: state.ui.albums
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(LibraryLocalAlbums)