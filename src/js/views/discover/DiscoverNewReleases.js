
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import Link from '../../components/Link';

import Header from '../../components/Header'
import Icon from '../../components/Icon'
import AlbumGrid from '../../components/AlbumGrid'
import Parallax from '../../components/Parallax'
import Thumbnail from '../../components/Thumbnail'
import ArtistSentence from '../../components/ArtistSentence'
import LazyLoadListener from '../../components/LazyLoadListener'

import * as helpers from '../../helpers'
import * as uiActions from '../../services/ui/actions'
import * as mopidyActions from '../../services/mopidy/actions'
import * as spotifyActions from '../../services/spotify/actions'

class DiscoverNewReleases extends React.Component{

	constructor(props){
		super(props);
	}

	componentDidMount(){
		this.props.uiActions.setWindowTitle("New releases");

		if (!this.props.new_releases){
			this.props.spotifyActions.getNewReleases();
		}
	}

	loadMore(){
		this.props.spotifyActions.getMore(
			this.props.new_releases_more,
			null,
			{
				type: 'SPOTIFY_NEW_RELEASES_LOADED'
			}
		);
	}

	playAlbum(e,album){
        this.props.mopidyActions.playURIs([album.uri],album.uri)
	}

	handleContextMenu(e,item){
		e.preventDefault()
		var data = { 
			e: e,
			context: 'album',
			uris: [item.uri],
			items: [item]
		}
		this.props.uiActions.showContextMenu(data)
	}

	renderIntro(album = null){
		if (album){
			return (
				<div className="intro preserve-3d">
					<Parallax image={album.images ? album.images.large : null} blur />
					<div className="content cf">
						<Link 
							to={global.baseURL+'album/'+album.uri}
							onContextMenu={e => this.handleContextMenu(e,album)}>
								<Thumbnail images={album.images} />
						</Link>
						<h2>
							<Link to={global.baseURL+'album/'+album.uri}>
								{album.name}
							</Link>
						</h2>
						<h3>
							<ArtistSentence artists={album.artists} />
						</h3>
						<div className="actions">
							<button className="primary" onClick={e => this.playAlbum(e,album)}>Play</button>
						</div>
					</div>
				</div>
			)
		} else {
			return (
				<div className="intro preserve-3d">
					<Parallax />
				</div>
			)
		}
	}

	render(){
		if (helpers.isLoading(this.props.load_queue,['spotify_browse/new-releases'])){
			return (
				<div className="view discover-new-releases-view">
					<Header>
						<Icon name="new_releases" type="material" />
						New releases
					</Header>
					<div className="body-loader loading">
						<div className="loader"></div>
					</div>
				</div>
			)
		}

		var albums = [];
		if (this.props.new_releases){
			for (var uri of this.props.new_releases){
				if (this.props.albums.hasOwnProperty(uri)){
					albums.push(this.props.albums[uri]);
				}
			}
		}

		// Pull the first playlist out and we'll use this as a banner
		var first_album = albums.splice(0,1)
		if (first_album){
			first_album = helpers.collate(first_album[0], {artists: this.props.artists});
		}

		var options = (
			<a className="button no-hover" onClick={e => {this.props.uiActions.hideContextMenu(); this.props.spotifyActions.getNewReleases()}}>
				<Icon name="refresh" />Refresh
			</a>
		);

		return (
			<div className="view discover-new-releases-view preserve-3d">
				<Header className="overlay" options={options} uiActions={this.props.uiActions}>
					<Icon name="new_releases" type="material" />
					New releases
				</Header>
				{this.renderIntro(first_album)}
				<section className="content-wrapper grid-wrapper">
					<AlbumGrid albums={albums} />
				</section>
				<LazyLoadListener
					loadKey={this.props.new_releases_more}
					showLoader={this.props.new_releases_more}
					loadMore={() => this.loadMore()}
				/>
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
		theme: state.ui.theme,
		load_queue: state.ui.load_queue,
		artists: state.core.artists,
		albums: state.core.albums,
		new_releases: state.spotify.new_releases,
		new_releases_more: state.spotify.new_releases_more,
		new_releases_total: state.spotify.new_releases_total
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		uiActions: bindActionCreators(uiActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(DiscoverNewReleases)