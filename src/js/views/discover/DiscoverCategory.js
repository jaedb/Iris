
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'

import Header from '../../components/Header'
import PlaylistGrid from '../../components/PlaylistGrid'
import LazyLoadListener from '../../components/LazyLoadListener'
import * as helpers from '../../helpers'
import * as spotifyActions from '../../services/spotify/actions'

class DiscoverCategory extends React.Component{

	constructor(props) {
		super(props);
	}

	componentDidMount(){
		this.loadCategory()
	}

	componentWillReceiveProps( nextProps ){
		if( nextProps.params.id != this.props.params.id ){
			this.loadCategory()
		}
	}

	loadCategory(){
		if (!this.props.category || !this.props.category.playlists_uris) this.props.spotifyActions.getCategory( this.props.params.id );
	}

	loadMore(){
		this.props.spotifyActions.getURL( this.props.category.playlists_more, 'SPOTIFY_CATEGORY_PLAYLISTS_LOADED', 'category:'+this.props.params.id );
	}

	render(){
		if (helpers.isLoading(this.props.load_queue,['spotify_browse/categories/'])){
			return (
				<div className="view discover-categories-view">
					<Header icon="grid" title={(this.props.category ? this.props.category.name : 'Category')} />
					<div className="body-loader">
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
				<Header icon="grid" title={this.props.category.name} />
				<section className="grid-wrapper">
					<PlaylistGrid playlists={playlists} />
				</section>
				<LazyLoadListener enabled={this.props.category.playlists_more} loadMore={ () => this.loadMore() }/>
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
		playlists: state.ui.playlists,
		category: (state.ui.categories && typeof(state.ui.categories['category:'+ownProps.params.id]) !== 'undefined' ? state.ui.categories['category:'+ownProps.params.id] : false )
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(DiscoverCategory)