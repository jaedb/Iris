
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link } from 'react-router'

import Header from '../../components/Header'

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
			this.loadDirectory();
		}

		// our uri changes
		if( nextProps.params.uri != this.props.params.uri ){
			this.loadDirectory(nextProps.params.uri);
		}
	}

	loadDirectory(uri = this.props.params.uri){
		if( this.props.mopidy.connected ){
			this.props.mopidyActions.getBrowse(uri);
		}
	}

	renderDirectory(){
		if( !this.props.mopidy.browse ) return null

		return (
			<div>
				{
					this.props.mopidy.browse.map( directory => {
						return (
							<div key={directory.uri}>
								<Link to={'/library/local/directory/'+encodeURIComponent(directory.uri)}>
									{ directory.name }
								</Link>
							</div>
						)
					})
				}
			</div>
		)
	}

	render(){
		return (
			<div className="view library-local-view">
				<Header icon="music" title="Local" />
				{ this.renderDirectory() }
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