
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import Header from '../../components/Header'
import Icon from '../../components/Icon'
import CategoryGrid from '../../components/CategoryGrid'

import * as helpers from '../../helpers'
import * as uiActions from '../../services/ui/actions'
import * as spotifyActions from '../../services/spotify/actions'

class DiscoverCategories extends React.Component{

	constructor(props){
		super(props);
	}

	componentDidMount(){

		// Check for an empty category index, or where we've only got one loaded
		// This would be the case if you've refreshed from within a category and only loaded
		// the single record.
		if (!this.props.categories || Object.keys(this.props.categories).length <= 1){
			this.props.spotifyActions.getCategories();
		}
		this.props.uiActions.setWindowTitle("Genre / Mood");
	}

	render(){
		if (helpers.isLoading(this.props.load_queue,['spotify_browse/categories'])){
			return (
				<div className="view discover-categories-view">
					<Header icon="grid" title="Genre / Mood" />
					<div className="body-loader loading">
						<div className="loader"></div>
					</div>
				</div>
			)
		}

		// convert categories object into simple array
		var categories = []
		if (this.props.categories){
			for (var key in this.props.categories){	
				if (this.props.categories.hasOwnProperty(key)){
					categories.push(this.props.categories[key])
				}
			}
		}

		return (
			<div className="view discover-categories-view">
				<Header uiActions={this.props.uiActions}>
					<Icon name="mood" type="material" />
					Genre / Mood
				</Header>
				<section className="content-wrapper grid-wrapper">
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
		categories: state.spotify.categories,
		load_queue: state.ui.load_queue
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(DiscoverCategories)