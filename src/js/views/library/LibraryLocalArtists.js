
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
		if( !this.props.mopidy.connected && nextProps.mopidy.connected ){
			this.loadArtists(nextProps);
		}
	}

	loadArtists(props = this.props){
		if( props.mopidy.connected ){
			this.props.mopidyActions.getArtists();
		}
	}

	render(){
		if( !this.props.mopidy.artists ) return null

		return (
			<div className="view library-local-view">
				<Header icon="music" title="Local artists" />
				<section className="list-wrapper">
					<List columns={[{ name: 'name', width: '100'}]} rows={this.props.mopidy.artists} link_prefix="/artist/" />
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

export default connect(mapStateToProps, mapDispatchToProps)(LibraryLocalArtists)