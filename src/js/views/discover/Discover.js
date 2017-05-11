
import React, { PropTypes } from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import FontAwesome from 'react-fontawesome'

import SidebarToggleButton from '../../components/SidebarToggleButton'
import GridSlider from '../../components/GridSlider'
import ArtistSentence from '../../components/ArtistSentence'
import TrackList from '../../components/TrackList'
import Parallax from '../../components/Parallax'
import AutocompleteField from '../../components/AutocompleteField'

import * as helpers from '../../helpers'
import * as spotifyActions from '../../services/spotify/actions'

class Discover extends React.Component{

	constructor(props) {
		super(props)

		this._autocomplete_timer = false

		this.state = {
			seeds: [],
			adding_seed: false
		}
	}

	componentDidMount(){
		this.props.spotifyActions.getGenres()
		if (this.props.authorized){
			this.props.spotifyActions.getFavorites()
		}
	}

	componentWillReceiveProps(newProps, newState){
		if (this.props.favorite_artists.length <= 0 && newProps.favorite_artists.length > 0 && this.state.seeds.length <= 0){
			var initial_seeds = newProps.favorite_artists.sort(() => .5 - Math.random())
			initial_seeds = initial_seeds.slice(0,2)

			this.setState({seeds: initial_seeds})
			this.getRecommendations(initial_seeds)
		}
	}

	getRecommendations(seeds = this.state.seeds){
		var uris = helpers.asURIs(seeds)
		this.props.spotifyActions.getRecommendations(uris)
	}

	removeSeed(index){
		var seeds = this.state.seeds
		seeds.splice(index,1)
		this.setState({seeds: seeds})
		this.getRecommendations(seeds)
	}

	handleSelect(item){
		var seeds = this.state.seeds
		seeds.push(item)
		this.setState({seeds: seeds})
		this.getRecommendations(seeds)
	}

	renderSeeds(){
		return (
			<div className="seeds">
				{
					this.state.seeds.map((seed,index) => {
						var type = helpers.uriType(seed.uri)
						if (!type){
							type = 'genre'
						}
						return (
							<span className="seed" key={seed.uri}>
								{seed.name}
								<span className="type">({type})</span>
								<FontAwesome name="close" className="remove" onClick={() => this.removeSeed(index)} />
							</span>
						)
					})
				}
				<AutocompleteField types={['artist','track','genre']} placeholder="Add seed" handleSelect={item => this.handleSelect(item)} clearOnSelect />
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
						<h1>Discover new music</h1>
						<h3>Add seeds below to build your sound</h3>
						{this.renderSeeds()}
					</div>
				</div>
				{this.props.recommendations ? <section className="list-wrapper"><TrackList className="discover-track-list" uri="iris:discover" tracks={this.props.recommendations} /></section> : null}
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
		authorized: state.spotify.authorized,
		quick_search_results: (state.spotify.quick_search_results ? state.spotify.quick_search_results : {artists: [], tracks: []}),
		recommendations: (state.spotify.recommendations ? state.spotify.recommendations : []),
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