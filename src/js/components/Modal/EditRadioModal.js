
import React, { PropTypes } from 'react'
import FontAwesome from 'react-fontawesome'

import Icon from '../Icon'
import * as helpers from '../../helpers'

export default class EditRadioModal extends React.Component{

	constructor(props){
		super(props)
		this.state = {
			enabled: false,
			seeds: [],
			uri: ''
		}
	}

	componentDidMount(){
		if (!this.props.radio || !this.props.radio.enabled) return null
		var seeds = [...this.props.radio.seed_tracks, ...this.props.radio.seed_artists, ...this.props.radio.seed_genres]
		this.setState({seeds: seeds, enabled: this.props.radio.enabled})

		this.props.spotifyActions.resolveRadioSeeds(this.props.radio)
	}

	handleSubmit(e){
		if (this.state.enabled){
			this.props.pusherActions.startRadio(this.state.seeds)
		}else{
			this.props.pusherActions.stopRadio()
		}
		this.props.uiActions.closeModal()
	}

	addSeed(){
		if (this.state.uri == '') return null

		var seeds = Object.assign([],this.state.seeds)
		var uris = this.state.uri.split(',')

		for (var i = 0; i < uris.length; i++){
			if (seeds.indexOf(uris[i]) <= -1){
				seeds.push(uris[i])
			} else {
				this.props.uiActions.createNotification(uris[i]+' already added','bad')
			}			
		}

		// commit to state
		this.setState({
			seeds: seeds,
			uri: ''
		})
	}

	removeSeed(uri){
		var seeds = []
		for (var i = 0; i < this.state.seeds.length; i++){
			if (this.state.seeds[i] != uri){
				seeds.push(this.state.seeds[i])
			}
		}
		this.setState({seeds: seeds})
	}

	renderSeeds(){
		var seeds = []

		if (this.state.seeds){
			for (var i = 0; i < this.state.seeds.length; i++){
				var uri = this.state.seeds[i]
				if (uri){
					if (helpers.uriType(uri) == 'artist' && this.props.artists){
						if (this.props.artists.hasOwnProperty(uri)){
							seeds.push(this.props.artists[uri])
						} else {
							seeds.push({
								type: 'artist',
								unresolved: true,
								uri: uri
							})
						}
					} else if (helpers.uriType(uri) == 'track' && this.props.tracks){
						if (this.props.tracks.hasOwnProperty(uri)){
							seeds.push(this.props.tracks[uri])
						} else {
							seeds.push({
								type: 'track',
								unresolved: true,
								uri: uri
							})
						}
					}
				}
			}
		}

		return (
			<div>
				<div className="list">
					{
						seeds.map((seed,index) => {
							return (
								<div className="list-item" key={seed.uri}>
									{seed.unresolved ? <span className="grey-text">{seed.uri}</span> : <span>{seed.name}</span> }
									<span className="grey-text">&nbsp;({seed.type})</span>
									<FontAwesome name="close" className="pull-right destructive" onClick={() => this.removeSeed(seed.uri)} />
								</div>
							)
						})
					}
				</div>
			</div>
		)
	}

	renderAddSeeds(){
		return (
			<div className="field no-top-margin">
				<input 
					type="text"
					placeholder="Comma-separated URIs"
					onChange={e => this.setState({uri: e.target.value})} 
					value={this.state.uri} />
				<button type="button" className="discrete" onClick={e => this.addSeed()}><FontAwesome name="plus" /></button>
			</div>
		)
	}

	render(){
		return (
			<div>
				<h4>Edit radio</h4>

				<form onSubmit={e => this.handleSubmit(e)}>
					<div className="field checkbox white">
						<label>
							<input 
								type="checkbox"
								name="enabled"
								checked={ this.state.enabled }
								onChange={ e => this.setState({ enabled: !this.state.enabled })} />
							<span className="label">Radio mode enabled</span>
						</label>
					</div>

					{this.state.enabled ? this.renderSeeds() : null}
					{this.state.enabled ? this.renderAddSeeds() : null}

					<div className="actions centered-text">
						<button type="submit" className="primary wide">Save</button>
					</div>
				</form>
			</div>
		)
	}
}