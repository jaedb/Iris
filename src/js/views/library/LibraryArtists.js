
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link } from 'react-router'

import * as mopidyActions from '../../services/mopidy/actions'
import * as spotifyActions from '../../services/spotify/actions'

class LibraryArtists extends React.Component{

	constructor(props) {
		super(props);
	}

	// on render
	componentDidMount(){
		this.props.spotifyActions.getLibraryArtists();
	}

	render(){
		if( this.props.spotify.libraryArtists ){
			return (
				<div>
					<h3>My artists</h3>
					<ul>
						{
							this.props.spotify.libraryArtists.artists.items.map( (artist, index) => {
								var link = '/artist/' + artist.uri;
								return <li key={index}><Link to={link}>{ artist.name }</Link></li>
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

export default connect(mapStateToProps, mapDispatchToProps)(LibraryArtists)