
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

	componentDidMount(){
		this.props.spotifyActions.getCategories();
	}

	render(){
		if (!this.props.categories) return null

		// convert categories object into simple array
		var categories = []
		for (var key in this.props.categories){	
			if (this.props.categories.hasOwnProperty(key)){
				categories.push(this.props.categories[key])
			}
		}

		return (
			<div className="view discover-categories-view">
				<Header icon="grid" title="Genre / Mood" />
				<section className="grid-wrapper">
					<CategoryGrid categories={categories} />
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
		categories: state.ui.categories
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(DiscoverCategories)