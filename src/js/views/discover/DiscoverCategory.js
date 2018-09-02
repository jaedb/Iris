
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import Header from '../../components/Header'
import Icon from '../../components/Icon'
import PlaylistGrid from '../../components/PlaylistGrid'
import LazyLoadListener from '../../components/LazyLoadListener'

import * as helpers from '../../helpers'
import * as uiActions from '../../services/ui/actions'
import * as spotifyActions from '../../services/spotify/actions'

class DiscoverCategory extends React.Component{

	constructor(props){
		super(props);
	}

	componentDidMount(){
		this.loadCategory();
		this.setWindowTitle();
	}

	componentWillReceiveProps(nextProps){
		if (nextProps.params.id != this.props.params.id){
			this.loadCategory();
		}

		if (!this.props.category && nextProps.category){
			this.setWindowTitle(nextProps.category);
		}
	}

	setWindowTitle(category = this.props.category){
		if (category){
			this.props.uiActions.setWindowTitle(category.name);
		} else{
			this.props.uiActions.setWindowTitle("Genre / Mood");
		}
	}

	loadCategory(){
		if (!this.props.category || !this.props.category.playlists_uris){
			this.props.spotifyActions.getCategory(this.props.params.id);
		}
	}

	loadMore(){
		this.props.spotifyActions.getMore(
			this.props.category.playlists_more,
			null,
			{
				type: 'SPOTIFY_CATEGORY_PLAYLISTS_LOADED',
				key: 'category:'+this.props.params.id
			}
		);
	}

	render(){
		if (helpers.isLoading(this.props.load_queue,['spotify_browse/categories/'])){
			return (
				<div className="view discover-categories-view">
					<Header>
						<Icon name="mood" type="material" />
						{(this.props.category ? this.props.category.name : 'Category')}
					</Header>
					<div className="body-loader loading">
						<div className="loader"></div>
					</div>
				</div>
			)
		}

		if (!this.props.category){
			return null
		}

		var playlists = []
		if (this.props.category.playlists_uris){
			for (var i = 0; i < this.props.category.playlists_uris.length; i++){
				var key = this.props.category.playlists_uris[i]
				if (this.props.playlists.hasOwnProperty(key)){
					playlists.push(this.props.playlists[key])
				}
			}
		}

		return (
			<div className="view discover-categories-view">
				<Header>
					<Icon name="mood" type="material" />
					{this.props.category.name}
				</Header>
				<div className="content-wrapper">
					<section className="grid-wrapper">
						<PlaylistGrid playlists={playlists} />
					</section>
					<LazyLoadListener 
						loadKey={this.props.category.playlists_more} 
						showLoader={this.props.category.playlists_more} 
						loadMore={() => this.loadMore()}
					/>
				</div>
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
		load_queue: state.ui.load_queue,
		playlists: state.core.playlists,
		category: (state.core.categories && state.core.categories['category:'+ownProps.params.id] !== undefined ? state.core.categories['category:'+ownProps.params.id] : false )
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(DiscoverCategory)