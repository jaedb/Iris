
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

	// on render
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
		if( !this.props.albums ) return null

		return (
			<div className="view library-local-view">
				<Header icon="music" title="Local albums" />
				<div>
					<List columns={[{ name: 'name', width: '100'}]} rows={this.props.albums} link_prefix="/album/" />
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
		albums: state.mopidy.albums
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(LibraryLocalAlbums)