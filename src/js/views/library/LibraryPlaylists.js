
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link } from 'react-router'

import * as mopidyActions from '../../services/mopidy/actions'
import * as spotifyActions from '../../services/spotify/actions'

class LibraryPlaylists extends React.Component{

	constructor(props) {
		super(props);
	}

	// on render
	componentDidMount(){
		this.props.spotifyActions.getLibraryPlaylists();
	}

	render(){
		if( this.props.spotify.libraryPlaylists ){
			return (
				<div>
					<h3>My playlists</h3>
					<ul>
						{
							this.props.spotify.libraryPlaylists.items.map( (playlist, index) => {
								var link = '/playlist/' + playlist.uri;
								return <li key={index}><Link to={link}>{ playlist.name }</Link></li>
							})
						}
					</ul>
				</div>
			);
		}
		return null;
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

export default connect(mapStateToProps, mapDispatchToProps)(LibraryPlaylists)