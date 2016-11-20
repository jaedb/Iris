
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

	// on render
	componentDidMount(){
		this.props.spotifyActions.getLibraryArtists();
	}

	loadMore(){
		if( !this.props.artists_more ) return
		this.props.spotifyActions.getURL( this.props.artists_more, 'SPOTIFY_LIBRARY_ARTISTS_LOADED_MORE' );
	}

	renderView(){
		if( !this.props.artists ) return null

		if( this.props.view == 'list' ){
			var columns = [
				{
					width: 30,
					name: 'name'
				},
				{
					width: 10,
					name: 'followers.total'
				}
			]
			return (
				<section className="list-wrapper">
					<List rows={this.props.artists} columns={columns} link_prefix="/artist/" />
				</section>
			)
		}else{
			return (
				<section className="grid-wrapper">
					<ArtistGrid artists={this.props.artists} />
				</section>				
			)
		}
	}

	render(){

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
			<DropdownField icon="eye" name="View" value={ this.props.view } options={ view_options } handleChange={ value => this.props.uiActions.setView({ library_artists_view: value }) } />
		)

		return (
			<div className="view library-artists-view">
				<Header icon="mic" title="My artists" actions={actions} />				
				{ this.renderView() }
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
		view: state.ui.library_artists_view,
		artists: state.spotify.library_artists,
		artists_more: state.spotify.library_artists_more
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