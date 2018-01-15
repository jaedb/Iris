
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'
import InputRange from 'react-input-range';

import Header from '../../components/Header'
import ArtistSentence from '../../components/ArtistSentence'
import ArtistGrid from '../../components/ArtistGrid'
import AlbumGrid from '../../components/AlbumGrid'
import TrackList from '../../components/TrackList'
import Parallax from '../../components/Parallax'
import AddSeedField from '../../components/AddSeedField'
import URILink from '../../components/URILink'
import ContextMenuTrigger from '../../components/ContextMenuTrigger'
import RelatedArtists from '../../components/RelatedArtists'

import * as helpers from '../../helpers'
import * as uiActions from '../../services/ui/actions'
import * as mopidyActions from '../../services/mopidy/actions'
import * as spotifyActions from '../../services/spotify/actions'

class Discover extends React.Component{

	constructor(props){
		super(props)

		this._autocomplete_timer = false

		this.state = {
			add_seed: '',
			adding_seed: false,
			seeds: [],
			tunabilities: {
				acousticness: {
					enabled: false,
					convert_to_decimal: true,
					min: 0,
					max: 100,
					value: {
						min: 0,
						max: 100
					}
				},
				danceability: {
					enabled: false,
					convert_to_decimal: true,
					min: 0,
					max: 100,
					value: {
						min: 0,
						max: 100
					}
				},
				energy: {
					enabled: false,
					convert_to_decimal: true,
					min: 0,
					max: 100,
					value: {
						min: 0,
						max: 100
					}
				},
				instrumentalness: {
					enabled: false,
					convert_to_decimal: true,
					min: 0,
					max: 100,
					value: {
						min: 0,
						max: 100
					}
				},
				key: {
					enabled: false,
					min: 0,
					max: 11,
					value: {
						min: 0,
						max: 11
					}
				},
				liveness: {
					enabled: false,
					convert_to_decimal: true,
					min: 0,
					max: 100,
					value: {
						min: 0,
						max: 100
					}
				},
				loudness: {
					enabled: false,
					convert_to_decimal: true,
					min: 0,
					max: 100,
					value: {
						min: 0,
						max: 100
					}
				},
				popularity: {
					enabled: true,
					min: 0,
					max: 100,
					value: {
						min: 0,
						max: 100
					}
				},
				speechiness: {
					enabled: false,
					convert_to_decimal: true,
					description: "The presence of spoken words in a track",
					min: 0,
					max: 100,
					value: {
						min: 0,
						max: 100
					}
				},
				tempo: {
					enabled: false,
					convert_to_decimal: true,
					min: 0,
					max: 100,
					value: {
						min: 0,
						max: 100
					}
				},
				valence: {
					enabled: false,
					convert_to_decimal: true,
					description: "The musical positiveness conveyed by a track",
					min: 0,
					max: 100,
					value: {
						min: 0,
						max: 100
					}
				}
			}
		}
	}

	componentDidMount(){

		// We have seeds provided in the URL
		if (this.props.params.seeds){
			this.handleURLSeeds(this.props.params.seeds);
		} 
	}

	componentWillReceiveProps(newProps, newState){

		// New seeds via URL
		if (newProps.params.seeds != this.props.params.seeds){
			this.handleURLSeeds(newProps.params.seeds)
		}
	}

	handleContextMenu(e){

		var uri = 'iris:discover';
		if (this.state.seeds){
			uri += ':';
			for (var i = 0; i < this.state.seeds.length; i++){
				if (i > 0){
					uri += ',';
				}
				uri += this.state.seeds[i].split(':').join('_');
			}
		}

		var tracks = [];
		if (this.props.recommendations.tracks_uris && this.props.tracks){
			for (var i = 0; i < this.props.recommendations.tracks_uris.length; i++){
				var uri = this.props.recommendations.tracks_uris[i];
				if (this.props.tracks.hasOwnProperty(uri)){
					tracks.push(this.props.tracks[uri]);
				}
			}
		}

		var data = {
			e: e,
			context: 'track',
			items: tracks,
			uris: [uri]
		}
		this.props.uiActions.showContextMenu(data);
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

	getRecommendations(seeds = this.state.seeds, tunabilities = this.state.tunabilities){
		if (seeds.length > 0){
			var digested_tunabilities = {};
			for (var key in tunabilities){
				if (tunabilities.hasOwnProperty(key) && tunabilities[key].enabled){
					var tunability = tunabilities[key];

					if (tunability.convert_to_decimal){
						tunability.value.max = tunability.max / 100;
						tunability.value.min = tunability.min / 100;
					}

					digested_tunabilities[key+"_max"] = tunability.value.max;
					digested_tunabilities[key+"_min"] = tunability.value.min;
				}
			}
			this.props.spotifyActions.getRecommendations(seeds, 50, digested_tunabilities)
		}
	}

	playTracks(){
		this.props.mopidyActions.playURIs(this.props.recommendations.tracks_uris);
	}

	removeSeed(index){
		var seeds = this.state.seeds;
		seeds.splice(index,1);
		this.setState({seeds: seeds});
	}

	handleSelect(e,uri){
		var seeds = this.state.seeds;
		seeds.push(uri);
		this.setState({seeds: seeds});
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
								<URILink className="type" type={type} uri={seed.uri}>({type})</URILink>
								<FontAwesome name="close" className="remove" onClick={() => this.removeSeed(index)} />
							</span>
						)
					})
				}
				<AddSeedField onSelect={(e,uri) => this.handleSelect(e,uri)} />
			</div>
		)
	}

	setTunability(name, value){
		var tunabilities = this.state.tunabilities;
		tunabilities[name].value = value;
		this.setState({ tunabilities: tunabilities });
	}

	toggleTunability(name){
		var tunabilities = this.state.tunabilities;
		tunabilities[name].enabled = !tunabilities[name].enabled;
		this.setState({ tunabilities: tunabilities });
	}

	renderTunabilities(){

		var tunabilities = [];
		for (var key in this.state.tunabilities){
			if (this.state.tunabilities.hasOwnProperty(key)){
				var tunability = Object.assign(
					{},
					this.state.tunabilities[key],
					{
						name: key
					}
				);
				tunabilities.push(tunability);
			}
		}

		return (
			<div className="tunabilities">
				{
					tunabilities.map(tunability => {
						return (
							<div className="field tunability" key={tunability.name}>
								<div className="field sub-field checkbox">
									<label>
										<input 
											type="checkbox"
											name={"tunability_"+tunability.name}
											checked={tunability.enabled}
											onChange={e => this.toggleTunability(tunability.name)} />
										<div className="label">
											{tunability.name}
										</div>
										<span className="has-tooltip info">
											<FontAwesome name="info-circle" />
											<span className="tooltip">{tunability.description}</span>
										</span>
									</label>
								</div>
								{tunability.enabled ? <div className="input">
									<div className="field sub-field range">
										<InputRange
											disabled={!tunability.enabled}
											minValue={tunability.min}
											maxValue={tunability.max}
											value={tunability.value}
											onChange={value => this.setTunability(tunability.name, value)}
										/>
									</div>
								</div> : null}
							</div>
						);
					})
				}
			</div>
		);
	}

	renderResults(){
		if (helpers.isLoading(this.props.load_queue,['spotify_recommendations'])){
			return (
				<div className="body-loader loading">
					<div className="loader"></div>
				</div>
			)
		}
		
		if (!this.props.recommendations || this.props.recommendations.albums_uris === undefined || this.props.recommendations.artists_uris === undefined){
			return null;
		}

		var tracks = [];
		if (this.props.recommendations.tracks_uris && this.props.tracks){
			for (var i = 0; i < this.props.recommendations.tracks_uris.length; i++){
				var uri = this.props.recommendations.tracks_uris[i];
				if (this.props.tracks.hasOwnProperty(uri)){
					tracks.push(this.props.tracks[uri]);
				}
			}
		}

		var artists = [];
		if (this.props.recommendations.artists_uris && this.props.artists){
			for (var i = 0; i < this.props.recommendations.artists_uris.length; i++){
				var uri = this.props.recommendations.artists_uris[i];
				if (this.props.artists.hasOwnProperty(uri)){
					artists.push(this.props.artists[uri]);
				}
			}
		}

		var albums = [];
		if (this.props.recommendations.albums_uris && this.props.albums){
			for (var i = 0; i < this.props.recommendations.albums_uris.length; i++){
				var uri = this.props.recommendations.albums_uris[i];
				if (this.props.albums.hasOwnProperty(uri)){
					albums.push(this.props.albums[uri]);
				}
			}
		}

		var uri = 'iris:discover';
		if (this.state.seeds){
			uri += ':';
			for (var i = 0; i < this.state.seeds.length; i++){
				if (i > 0){
					uri += ',';
				}
				uri += this.state.seeds[i].split(':').join('_');
			}
		}
		
		return (
			<div className="content-wrapper recommendations-results cf">

				<section className="col w70">
					<h4>
						Tracks
						<ContextMenuTrigger onTrigger={e => this.handleContextMenu(e)} />
						<button className="primary pull-right" onClick={e => this.playTracks(e)}>Play all</button>
					</h4>
					<TrackList className="discover-track-list" uri={uri} tracks={tracks} />
				</section>

				<div className="col w5"></div>

				<div className="col w25">
					<section>
						<h4>Artists</h4>
						<RelatedArtists artists={artists} />
					</section>
					<br />
					<br />
					<section>
						<h4>Albums</h4>
						<AlbumGrid className="mini" albums={albums} />
					</section>
				</div>

			</div>
		)
	}

	render(){
		return (
			<div className="view discover-view">
				<div className="intro">

					<Parallax image="assets/backgrounds/discover.jpg" />

					<div className="liner">
						<h1>Explore new music</h1>
						<h2 className="grey-text">
							Add seeds and musical properties below to build your sound
						</h2>
						{this.renderSeeds()}
						{this.renderTunabilities()}
						<span className="button primary" onClick={e => this.getRecommendations()}>Apply</span>
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
		albums: state.core.albums,
		artists: state.core.artists,
		tracks: state.core.tracks,
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
		uiActions: bindActionCreators(uiActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(Discover)