
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link } from 'react-router'
import FontAwesome from 'react-fontawesome'

import PlaylistGrid from '../../components/PlaylistGrid'
import List from '../../components/List'
import DropdownField from '../../components/DropdownField'
import Header from '../../components/Header'

import * as uiActions from '../../services/ui/actions'
import * as mopidyActions from '../../services/mopidy/actions'
import * as spotifyActions from '../../services/spotify/actions'

class LibraryPlaylists extends React.Component{

	constructor(props) {
		super(props);
	}

	renderView(){
		if( !this.props.playlists ) return null

		if( this.props.view == 'list' ){
			var columns = [
				{
					width: 30,
					label: 'Name',
					name: 'name'
				},
				{
					width: 30,
					label: 'Owner',
					name: 'owner.id'
				},
				{
					width: 10,
					label: 'Can edit',
					name: 'can_edit'
				},
				{
					width: 10,
					label: 'Tracks',
					name: 'tracks.total'
				}
			]
			return (
				<section className="list-wrapper">
					<List rows={this.props.playlists} columns={columns} link_prefix="/playlist/" show_source_icon={true} />
				</section>
			)
		}else{
			return (
				<section className="grid-wrapper">
					<PlaylistGrid playlists={this.props.playlists} />
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
			<div>
				<DropdownField icon="eye" name="View" value={ this.props.view } options={ view_options } handleChange={ value => this.props.uiActions.setView({ library_playlists_view: value }) } />
				<button onClick={ () => this.props.uiActions.openModal('create_playlist', {} ) }>
					<FontAwesome name="plus" />&nbsp;
					New
				</button>
			</div>
		)

		return (
			<div className="view library-playlists-view">
				<Header icon="playlist" title="My playlists" actions={actions} />
				{ this.renderView() }
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
		view: state.ui.library_playlists_view,
		playlists: state.ui.playlists
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(LibraryPlaylists)