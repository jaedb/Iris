
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Link } from 'react-router'

import AlbumGrid from '../../components/AlbumGrid'
import List from '../../components/List'
import Header from '../../components/Header'
import Thumbnail from '../../components/Thumbnail'
import TrackList from '../../components/TrackList'
import ArtistSentence from '../../components/ArtistSentence'
import DropdownField from '../../components/DropdownField'
import LazyLoadListener from '../../components/LazyLoadListener'

import * as uiActions from '../../services/ui/actions'
import * as mopidyActions from '../../services/mopidy/actions'
import * as spotifyActions from '../../services/spotify/actions'

class LibraryAlbums extends React.Component{

	constructor(props) {
		super(props)
	}

	componentDidMount(){
		this.props.spotifyActions.getLibraryAlbums();
	}

	loadMore(){
		if( !this.props.albums_more ) return
		this.props.spotifyActions.getURL( this.props.albums_more, 'SPOTIFY_LIBRARY_ALBUMS_LOADED_MORE' );
	}

	renderView(){
		if( !this.props.albums ) return null

		if( this.props.view == 'list' ){
			var columns = [
				{
					width: 35,
					label: 'Name',
					name: 'name'
				},
				{
					width: 35,
					label: 'Artists',
					name: 'artists'
				},
				{
					width: 15,
					label: 'Added',
					name: 'added_at'
				},
				{
					width: 15,
					label: 'Tracks',
					name: 'tracks.total'
				}
			]
			return (
				<section className="list-wrapper">
					<List rows={this.props.albums} columns={columns} link_prefix="/album/" />
				</section>
			)
		}else if( this.props.view == 'thumbnails' ){
			return (
				<section className="grid-wrapper">
					{ this.props.albums ? <AlbumGrid albums={this.props.albums} /> : null }
				</section>			
			)
		}else{
			return (
				<section className="grid-wrapper albums-detail-subview">
					{
						this.props.albums.map( album => {
							return (
								<div className="album" key={album.uri}>
									<Thumbnail size="medium" images={album.images} />
									<div className="detail">
										<h3>{ album.name }</h3>
										<h4><ArtistSentence className="grey-text" artists={album.artists} /></h4>
										<TrackList tracks={album.tracks.items} />
									</div>
								</div>
							)
						})
					}
				</section>			
			)
		}
	}

	render(){

		var view_options = [
			{
				value: 'detail',
				label: 'Detail'
			},
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
			<DropdownField icon="eye" name="View" value={ this.props.view } options={ view_options } handleChange={ value => this.props.uiActions.set({ library_albums_view: value }) } />
		)

		return (
			<div className="view library-albums-view">
				<Header icon="cd" title="My albums" actions={actions} />
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
		view: state.ui.library_albums_view,
		albums: state.spotify.library_albums,
		albums_more: state.spotify.library_albums_more,
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(LibraryAlbums)