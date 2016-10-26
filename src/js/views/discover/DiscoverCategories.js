
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import Header from '../../components/Header'
import CategoryGrid from '../../components/CategoryGrid'

import * as spotifyActions from '../../services/spotify/actions'

class DiscoverCategories extends React.Component{

	constructor(props) {
		super(props);
	}

	// on render
	componentDidMount(){
		this.props.spotifyActions.getCategories();
	}

	render(){
		return (
			<div className="view discover-categories-view">
				<Header icon="grid" title="Genre / Mood" />
				{ this.props.spotify.categories ? <CategoryGrid categories={this.props.spotify.categories.items} /> : null }
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
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(DiscoverCategories)