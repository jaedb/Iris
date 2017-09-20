
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'

import Header from '../../components/Header'
import ArtistSentence from '../../components/ArtistSentence'
import ArtistGrid from '../../components/ArtistGrid'
import AlbumGrid from '../../components/AlbumGrid'
import TrackList from '../../components/TrackList'
import Parallax from '../../components/Parallax'
import AddSeedField from '../../components/AddSeedField'
import SidebarToggleButton from '../../components/SidebarToggleButton'
import * as helpers from '../../helpers'
import * as spotifyActions from '../../services/spotify/actions'

class Discover extends React.Component{

	constructor(props) {
		super(props)

		this._autocomplete_timer = false

		this.state = {
			seeds: [
				'spotify:genre:chill'
			],
			add_seed: '',
			adding_seed: false
		}
	}

	componentDidMount(){

		// We have seeds provided in the URL
		if (this.props.params.seeds){
			this.handleURLSeeds(this.props.params.seeds)

		// BAU
		} else {
			this.props.spotifyActions.getFavorites()
		}
	}

	componentWillReceiveProps(newProps, newState){

		// New seeds via URL
		if (newProps.params.seeds != this.props.params.seeds){
			this.handleURLSeeds(newProps.params.seeds)
		}

		// When we've loaded favorite_artists.
		// This indirectly listens for when the action has
		// loaded new data.
		if (this.props.favorite_artists.length <= 0 && newProps.favorite_artists.length > 0){
			var initial_seeds = newProps.favorite_artists.sort(() => .5 - Math.random())
			initial_seeds = initial_seeds.slice(0,2)

			this.setState({seeds: initial_seeds})
			this.getRecommendations(initial_seeds)
		}
	}

	handleURLSeeds(seeds_string = this.props.params.seeds){

		// Rejoin if we've had to uri-encode these as strings
		// We'd need to do this if our URL has been encoded so the whole URL can become
		// it's own URI (eg iris:discover:spotify_artist_1234) where we can't use ":"
		var seeds = seeds_string.split('_').join(':').split(',')

		for (var i = 0; i < seeds.length; i++){
			switch (helpers.uriType(seeds[i])){
				
				case 'artist':
					this.props.spotifyActions.getArtist(seeds[i])
					break

				case 'track':
					this.props.spotifyActions.getTrack(seeds[i])
					break
			}
		}
		
		this.setState({seeds: seeds})
		this.getRecommendations(seeds)
	}

	getRecommendations(seeds = this.state.seeds){
		if (seeds.length > 0){
			this.props.spotifyActions.getRecommendations(seeds, 50)
		}
	}

	removeSeed(index){
		var seeds = this.state.seeds
		seeds.splice(index,1)
		this.setState({seeds: seeds})
		this.getRecommendations(seeds)
	}

	handleSelect(e,uri){
		var seeds = this.state.seeds
		seeds.push(uri)
		this.setState({seeds: seeds})
		this.getRecommendations(seeds)
	}

	renderSeeds(){
		var seeds_objects = []

		if (this.state.seeds.length > 0){
			for (var i = 0; i < this.state.seeds.length; i++){
				var uri = this.state.seeds[i]

				switch (helpers.uriType(uri)){

					case 'track':
						if (typeof(this.props.tracks[uri]) !== 'undefined'){
							seeds_objects.push(this.props.tracks[uri])
						} else {
							seeds_objects.push({
								name: 'Loading...',
								uri: uri
							})
						}
						break

					case 'artist':
						if (typeof(this.props.artists[uri]) !== 'undefined'){
							seeds_objects.push(this.props.artists[uri])
						} else {
							seeds_objects.push({
								name: 'Loading...',
								uri: uri
							})
						}
						break

					case 'genre':
						var name = helpers.getFromUri('genreid',uri)
						seeds_objects.push({
							name: (name.charAt(0).toUpperCase() + name.slice(1)).replace('-',' '),
							uri: uri
						})
						break
				}
			}
		}

		return (
			<div className="seeds">
				{
					seeds_objects.map((seed,index) => {
						var type = helpers.uriType(seed.uri)
						return (
							<span className="seed" key={seed.uri}>
								{seed.name}
								<span className="type">({type})</span>
								<FontAwesome name="close" className="remove" onClick={() => this.removeSeed(index)} />
							</span>
						)
					})
				}
				<AddSeedField onSelect={(e,uri) => this.handleSelect(e,uri)} />
			</div>
		)
	}

	renderResults(){
		if (helpers.isLoading(this.props.load_queue,['spotify_recommendations'])){
			return (
				<div className="body-loader loading">
					<div className="loader"></div>
				</div>
			)
		}
		
		if (!this.props.recommendations || typeof(this.props.recommendations.albums_uris) === 'undefined' || typeof(this.props.recommendations.artists_uris) === 'undefined'){
			return null
		}

		var albums = []
		if (this.props.recommendations.albums_uris && this.props.albums){
			for (var i = 0; i < this.props.recommendations.albums_uris.length; i++){
				var uri = this.props.recommendations.albums_uris[i]
				if (this.props.albums.hasOwnProperty(uri)){
					albums.push(this.props.albums[uri])
				}
			}
		}

		var artists = []
		if (this.props.recommendations.artists_uris && this.props.artists){
			for (var i = 0; i < this.props.recommendations.artists_uris.length; i++){
				var uri = this.props.recommendations.artists_uris[i]
				if (this.props.artists.hasOwnProperty(uri)){
					artists.push(this.props.artists[uri])
				}
			}
		}

		var uri = 'iris:discover'
		if (this.props.params.seeds){
			uri += ':'+this.props.params.seeds.split(':').join('_')
		}
		
		return (
			<div className="content-wrapper recommendations-results">
				<section>
					<h4>Artists</h4>
					<ArtistGrid single_row artists={artists} />
				</section>
				<section>
					<h4>Albums</h4>
					<AlbumGrid single_row albums={albums} />
				</section>
				<section>
					<h4>Tracks</h4>
					{this.props.recommendations.tracks ? <TrackList className="discover-track-list" uri={uri} tracks={this.props.recommendations.tracks} /> : null}
				</section>
			</div>
		)
	}

	render(){
		return (
			<div className="view discover-view">
			
				<SidebarToggleButton />

				<div className="intro">

					<Parallax image="/iris/assets/backgrounds/discover.jpg" />

					<div className="liner">
						<h1>Explore new music</h1>
						<h2 className="grey-text">
							Add seeds below to build your sound
							{!this.props.params.seeds ? ". Let's start with two of your favorite artists." : null}
						</h2>
						{this.renderSeeds()}
					</div>

				</div>

				{this.renderResults()}

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
		albums: (state.core.albums ? state.core.albums : []),
		artists: (state.core.artists ? state.core.artists : []),
		tracks: (state.core.tracks ? state.core.tracks : []),
		genres: (state.core.genres ? state.core.genres : []),
		authorized: state.spotify.authorization,
		load_queue: state.ui.load_queue,
		quick_search_results: (state.spotify.quick_search_results ? state.spotify.quick_search_results : {artists: [], tracks: []}),
		recommendations: (state.spotify.recommendations ? state.spotify.recommendations : {}),
		favorite_artists: (state.spotify.favorite_artists ? state.spotify.favorite_artists : []),
		favorite_tracks: (state.spotify.favorite_tracks ? state.spotify.favorite_tracks : [])
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Discover)