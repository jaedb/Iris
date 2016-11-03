
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link } from 'react-router'

import Header from '../../components/Header'
import List from '../../components/List'

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
		if( !this.props.mopidy.connected && nextProps.mopidy.connected ){
			this.loadDirectory( nextProps );
		}

		// our uri changes
		if( nextProps.params.uri != this.props.params.uri ){
			this.loadDirectory( nextProps );
		}
	}

	loadDirectory( props = this.props ){
		if( props.mopidy.connected ){
			this.props.mopidyActions.getDirectory( props.params.uri );
		}
	}

	render(){
		if( !this.props.mopidy.directory ) return null

		return (
			<div className="view library-local-view">
				<Header icon="music" title="Local files" />
				<section className="list-wrapper">
					<List columns={[{ name: 'name', width: '100'}]} rows={this.props.mopidy.directory} link_prefix="/library/local/directory/" />
				</section>
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
	return state;
}

const mapDispatchToProps = (dispatch) => {
	return {
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(LibraryLocalDirectory)