
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
		if (!this.props.library_albums) this.props.spotifyActions.getLibraryAlbums();
	}

	loadMore(){
		if( !this.props.library_albums_more ) return
		this.props.spotifyActions.getURL( this.props.library_albums_more, 'SPOTIFY_LIBRARY_ALBUMS_LOADED' );
	}

	renderView(albums){
		if (!albums || albums.length <= 0) return null

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
					<List rows={albums} columns={columns} link_prefix={global.baseURL+"album/"} />
				</section>
			)
		}else if( this.props.view == 'thumbnails' ){
			return (
				<section className="grid-wrapper">
					<AlbumGrid albums={albums} />
				</section>			
			)
		}else{
			return (
				<section className="grid-wrapper albums-detail-subview">
					{
						albums.map( album => {
							return (
								<div className="album" key={album.uri}>
									<Link to={global.baseURL+'album/'+album.uri}>
										<Thumbnail size="medium" images={album.images} />
									</Link>
									<div className="detail">
										<Link to={global.baseURL+'album/'+album.uri}>
											<h3>{ album.name }</h3>
										</Link>
										<h4><ArtistSentence className="grey-text" artists={album.artists} /></h4>
									</div>
									<TrackList tracks={album.tracks} />
								</div>
							)
						})
					}
				</section>			
			)
		}
	}

	render(){
		var albums = []

		if (this.props.library_albums && this.props.albums){
			for (var i = 0; i < this.props.library_albums.length; i++){
				var uri = this.props.library_albums[i]
				if (this.props.albums.hasOwnProperty(uri)){
					albums.push(this.props.albums[uri])
				}
			}
		}

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
				{ this.renderView(albums) }
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
		albums: state.ui.albums,
		library_albums: state.ui.library_albums,
		library_albums_more: state.ui.library_albums_more,
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