
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link } from 'react-router'

import LazyLoadListener from '../../components/LazyLoadListener'
import Header from '../../components/Header'
import ArtistGrid from '../../components/ArtistGrid'
import List from '../../components/List'
import DropdownField from '../../components/DropdownField'

import * as uiActions from '../../services/ui/actions'
import * as mopidyActions from '../../services/mopidy/actions'
import * as spotifyActions from '../../services/spotify/actions'

class LibraryArtists extends React.Component{

	constructor(props) {
		super(props);
	}

	componentDidMount(){
		if (!this.props.library_artists) this.props.spotifyActions.getLibraryArtists();
	}

	loadMore(){
		if( !this.props.library_artists_more ) return
		this.props.spotifyActions.getURL( this.props.library_artists_more, 'SPOTIFY_LIBRARY_ARTISTS_LOADED' );
	}

	renderView(artists){
		if( this.props.view == 'list' ){
			var columns = [
				{
					width: 30,
					label: 'Name',
					name: 'name'
				},
				{
					width: 10,
					label: 'Followers',
					name: 'followers.total'
				}
			]
			return (
				<section className="list-wrapper">
					<List rows={artists} columns={columns} link_prefix={global.baseURL+"artist/"} show_source_icon={true} />
				</section>
			)
		}else{
			return (
				<section className="grid-wrapper">
					<ArtistGrid artists={artists} />
				</section>				
			)
		}
	}

	render(){

		var artists = []
		if (this.props.library_artists && this.props.artists){
			for (var i = 0; i < this.props.library_artists.length; i++){
				var uri = this.props.library_artists[i]
				if (this.props.artists.hasOwnProperty(uri)){
					artists.push(this.props.artists[uri])
				}
			}
		}

		var view_options = [
			{
				value: 'thumbnails',
				label: 'Thumbnails'
			},
			{
				value: 'list',
				label: 'List'
			}
		]

		var actions = (
			<DropdownField icon="eye" name="View" value={ this.props.view } options={ view_options } handleChange={ value => this.props.uiActions.set({ library_artists_view: value }) } />
		)

		return (
			<div className="view library-artists-view">
				<Header icon="mic" title="My artists" actions={actions} />				
				{ this.renderView(artists) }
				<LazyLoadListener loadMore={ () => this.loadMore() }/>
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
		artists: state.ui.artists,
		library_artists: state.ui.library_artists,
		library_artists_more: state.ui.library_artists_more,
		view: state.ui.library_artists_view
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(LibraryArtists)