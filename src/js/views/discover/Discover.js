
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import Header from '../../components/Header'

import * as helpers from '../../helpers'
import * as spotifyActions from '../../services/spotify/actions'

class Discover extends React.Component{

	constructor(props) {
		super(props);
	}

	render(){
		return (
			<div className="view discover-view">
				<Header icon="compass" title="Discover" />
				<h1>To come</h1>
			</div>
		)
	}
}


/**
 * Export our component
 *
 * We also integrate our global store, using connect()
 **/

const mapStateToProps = (state, ownProps) => {
	return {

	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Discover)