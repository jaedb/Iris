
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
			uri: '',
			error_message: null
		}
	}

	componentDidMount(){
		if (!this.props.radio || !this.props.radio.enabled) return null
		var seeds = [...this.props.radio.seed_tracks, ...this.props.radio.seed_artists, ...this.props.radio.seed_genres]
		this.setState({seeds: seeds, enabled: this.props.radio.enabled})

		this.props.spotifyActions.resolveRadioSeeds(this.props.radio)
	}

	handleStart(e){
		e.preventDefault()
		this.props.pusherActions.startRadio(this.state.seeds)
		this.props.uiActions.closeModal()
	}

	handleUpdate(e){
		e.preventDefault()
		this.props.pusherActions.updateRadio(this.state.seeds)
		this.props.uiActions.closeModal()
	}

	handleStop(e){
		e.preventDefault()
		this.props.pusherActions.stopRadio()
		this.props.uiActions.closeModal()
	}

	addSeed(){
		if (this.state.uri == ''){
			this.setState({error_message: 'Cannot be empty'})
			return null
		}

		var seeds = Object.assign([],this.state.seeds)
		var uris = this.state.uri.split(',')

		for (var i = 0; i < uris.length; i++){
			if (seeds.indexOf(uris[i]) > -1){
				this.setState({error_message: 'URI already added'})
			} else {
				seeds.push(uris[i])
				this.setState({error_message: null})
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
					if (helpers.uriType(uri) == 'artist'){
						if (this.props.artists && this.props.artists.hasOwnProperty(uri)){
							seeds.push(this.props.artists[uri])
						} else {
							seeds.push({
								type: 'artist',
								unresolved: true,
								uri: uri
							})
						}
					} else if (helpers.uriType(uri) == 'track'){
						if (this.props.tracks && this.props.tracks.hasOwnProperty(uri)){
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

		if (seeds.length > 0){
			return (
				<div>
					<div className="list">
						{
							seeds.map((seed,index) => {
								return (
									<div className="list-item" key={seed.uri}>
										{seed.unresolved ? <span className="grey-text">{seed.uri}</span> : <span>{seed.name}</span> }
										<span className="grey-text">&nbsp;({seed.type})</span>
										<button className="discrete remove-uri"  onClick={e => this.removeSeed(seed.uri)}>
											<FontAwesome name="close" />&nbsp;Remove
										</button>
									</div>
								)
							})
						}
					</div>
				</div>
			)
		} else {
			return (
				<div>
					<div className="list">
						<div className="list-item no-click">
							<span className="grey-text">No seeds</span>
						</div>
					</div>
				</div>
			)
		}
	}

	render(){
		return (
			<div>
				<h4>Manage radio</h4>

				<form>
					{this.renderSeeds()}

					<div className="field">
						<input 
							type="text"
							placeholder="Comma-separated URIs"
							onChange={e => this.setState({uri: e.target.value, error_message: null})} 
							value={this.state.uri} />
						<button className="discrete add-uri" onClick={e => this.addSeed()}>
							<FontAwesome name="plus" />&nbsp; Add
						</button>
						{this.state.error_message ? <span className="error">{this.state.error_message}</span> : null}
					</div>

					<div className="actions centered-text">
						{this.state.enabled ? <button className="destructive wide" onClick={e => this.handleStop(e)}>Stop</button> : null}
						{this.state.enabled ? <button className="primary wide" onClick={e => this.handleUpdate(e)}>Save</button> : <button className="primary wide" onClick={e => this.handleStart(e)}>Start</button>}
					</div>
				</form>
			</div>
		)
	}
}