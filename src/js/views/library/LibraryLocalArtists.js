
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import Header from '../../components/Header'
import List from '../../components/List'

import * as mopidyActions from '../../services/mopidy/actions'
import * as spotifyActions from '../../services/spotify/actions'

class LibraryLocalArtists extends React.Component{

	constructor(props) {
		super(props);
	}

	// on render
	componentDidMount(){
		this.loadArtists()
	}

	componentWillReceiveProps( nextProps ){
		if( !this.props.mopidy_connected && nextProps.mopidy_connected ){
			this.loadArtists(nextProps);
		}
	}

	loadArtists(props = this.props){
		if( props.mopidy_connected && !props.local_artists ){
			this.props.mopidyActions.getArtists();
		}
	}

	render(){
		var artists = []
		if (this.props.artists && this.props.local_artists){
			for (var i = 0; i < this.props.local_artists.length; i++){
				var uri = this.props.local_artists[i]
				if (this.props.artists.hasOwnProperty(uri)){
					artists.push(this.props.artists[uri])
				}
			}
		}

		return (
			<div className="view library-local-view">
				<Header icon="music" title="Local artists" />
				<section className="list-wrapper">
					<List columns={[{ name: 'name', width: '100'}]} rows={artists} link_prefix={global.baseURL+"artist/"} />
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
	return {
		mopidy_connected: state.mopidy.connected,
		local_artists: state.ui.local_artists,
		artists: state.ui.artists
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(LibraryLocalArtists)