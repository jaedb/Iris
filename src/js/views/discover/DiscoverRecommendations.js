
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import InputRange from 'react-input-range';

import Header from '../../components/Header'
import ArtistSentence from '../../components/ArtistSentence'
import ArtistGrid from '../../components/ArtistGrid'
import AlbumGrid from '../../components/AlbumGrid'
import TrackList from '../../components/TrackList'
import Thumbnail from '../../components/Thumbnail'
import Parallax from '../../components/Parallax'
import DropdownField from '../../components/Fields/DropdownField'
import AddSeedField from '../../components/Fields/AddSeedField'
import URILink from '../../components/URILink'
import ContextMenuTrigger from '../../components/ContextMenuTrigger'
import RelatedArtists from '../../components/RelatedArtists'
import Icon from '../../components/Icon'

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
						min: 25,
						max: 75
					}
				},
				danceability: {
					enabled: false,
					convert_to_decimal: true,
					min: 0,
					max: 100,
					value: {
						min: 25,
						max: 75
					}
				},
				energy: {
					enabled: false,
					convert_to_decimal: true,
					min: 0,
					max: 100,
					value: {
						min: 25,
						max: 75
					}
				},
				instrumentalness: {
					enabled: false,
					convert_to_decimal: true,
					min: 0,
					max: 100,
					value: {
						min: 25,
						max: 75
					}
				},
				key: {
					enabled: false,
					min: 0,
					max: 11,
					value: {
						min: 3,
						max: 8
					}
				},
				liveness: {
					enabled: false,
					convert_to_decimal: true,
					min: 0,
					max: 100,
					value: {
						min: 25,
						max: 75
					}
				},
				loudness: {
					enabled: false,
					convert_to_decimal: true,
					min: 0,
					max: 100,
					value: {
						min: 25,
						max: 75
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
						min: 25,
						max: 75
					}
				},
				tempo: {
					enabled: false,
					convert_to_decimal: true,
					min: 0,
					max: 100,
					value: {
						min: 25,
						max: 75
					}
				},
				valence: {
					enabled: false,
					convert_to_decimal: true,
					description: "The musical positiveness conveyed by a track",
					min: 0,
					max: 100,
					value: {
						min: 25,
						max: 75
					}
				}
			}
		}
	}

	componentDidMount(){
		this.props.uiActions.setWindowTitle("Discover");

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
			uris: helpers.arrayOf('uri',tracks)
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

					var max = tunability.value.max;
					var min = tunability.value.min;

					if (tunability.convert_to_decimal){
						max = max / 100;
						min = min / 100;
					}

					digested_tunabilities[key+"_max"] = max.toString();
					digested_tunabilities[key+"_min"] = min.toString();
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
							seeds_objects.push(this.props.tracks[uri]);
						} else {
							seeds_objects.push({
								name: 'Loading...',
								uri: uri
							})
						}
						break

					case 'artist':
						if (typeof(this.props.artists[uri]) !== 'undefined'){
							seeds_objects.push(this.props.artists[uri]);
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
							<div className={"seed"+(seed.images ? " has-thumbnail" : "")} key={seed.uri}>
								{seed.images ? <URILink className="thumbnail-wrapper" type={type} uri={seed.uri}><Thumbnail images={seed.images} circle={seed.type == "artist"} size="small" /></URILink> : null}
								<div className="label">
									{helpers.titleCase(type)}
									<Icon name="close" className="remove" onClick={() => this.removeSeed(index)} />
								</div>
								<div className="name">{seed.name}</div>
							</div>
						)
					})
				}
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
		var addable_tunabilities = [];
		var enabled_tunabilities = [];
		for (var key in this.state.tunabilities){
			if (this.state.tunabilities.hasOwnProperty(key)){

				var tunability = Object.assign(
					{},
					this.state.tunabilities[key],
					{
						name: key
					}
				);

				if (tunability.enabled){
					enabled_tunabilities.push(tunability);
				} else {
					addable_tunabilities.push({
						label: helpers.titleCase(tunability.name),
						value: tunability.name
					});
				}
			}
		}

		return (
			<div className="tunabilities">
				{
					enabled_tunabilities.map(tunability => {
						return (
							<div className="field tunability range" key={tunability.name}>
								<div className="label">
									{helpers.titleCase(tunability.name)}
									<span className="remove" onClick={e => this.toggleTunability(tunability.name)}>
										<Icon name="close" />
									</span>
								</div>
								<div className="input">
									<InputRange
										disabled={!tunability.enabled}
										minValue={tunability.min}
										maxValue={tunability.max}
										value={tunability.value}
										onChange={value => this.setTunability(tunability.name, value)}
									/>
								</div>
							</div>
						);
					})
				}
			</div>
		);
	}

	renderResults(){

		// Results not in
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

		// Complete records not yet in our index
		if (tracks.length <= 0 && artists.length <= 0 && albums.length <= 0){
			return null;
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

				<section className="col w70 tracks">
					<h4>
						Tracks
						<ContextMenuTrigger onTrigger={e => this.handleContextMenu(e)} />
						<button className="primary pull-right" onClick={e => this.playTracks(e)}>Play all</button>
					</h4>
					<TrackList className="discover-track-list" uri={uri} tracks={tracks} />
				</section>

				<div className="col w5"></div>

				<div className="col w25 others">
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
		var is_loading = helpers.isLoading(this.props.load_queue,['spotify_recommendations']);
		var addable_tunabilities = [];
		for (var key in this.state.tunabilities){
			if (this.state.tunabilities.hasOwnProperty(key)){

				var tunability = Object.assign(
					{},
					this.state.tunabilities[key],
					{
						name: key
					}
				);

				if (!tunability.enabled){
					addable_tunabilities.push({
						label: helpers.titleCase(tunability.name),
						value: tunability.name
					});
				}
			}
		}

		return (
			<div className="view discover-view">
				<div className="intro">

					<Parallax image="assets/backgrounds/discover.jpg" theme={this.props.theme} />

					<div className="liner">
						<h1>Explore new music</h1>
						<h2 className="grey-text">
							Add seeds and musical properties below to build your sound
						</h2>
						<div className="parameters">

							{this.renderSeeds()}
							{this.renderTunabilities()}
							{this.state.seeds.length > 5 ? <p className="message error">Too many seeds! You can use up to a total of 5 seed tracks, artists and genres.</p> : null}

						</div>
						<div className="actions">

							<AddSeedField onSelect={(e,uri) => this.handleSelect(e,uri)} />
							<DropdownField className="add-properties" name="Properties" options={addable_tunabilities} no_status_icon button="default" handleChange={val => {this.toggleTunability(val)}} />
							<span className={"submit button primary large"+(is_loading ? " working" : "")} onClick={e => this.getRecommendations()}>
								<Icon name="explore" />&nbsp; 
								Find recommendations
							</span>

						</div>
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
		theme: state.ui.theme,
		albums: state.core.albums,
		artists: state.core.artists,
		tracks: state.core.tracks,
		genres: (state.core.genres ? state.core.genres : []),
		authorized: state.spotify.authorization,
		load_queue: state.ui.load_queue,
		quick_search_results: (state.spotify.quick_search_results ? state.spotify.quick_search_results : {artists: [], tracks: []}),
		recommendations: (state.spotify.recommendations ? state.spotify.recommendations : {})
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