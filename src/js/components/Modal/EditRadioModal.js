
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
			uri_validated: false
		}
	}

	componentDidMount(){
		if (!this.props.radio || !this.props.radio.enabled) return null
		var seeds = [...this.props.radio.seed_tracks, ...this.props.radio.seed_artists, ...this.props.radio.seed_genres]
		this.setState({seeds: seeds, enabled: this.props.radio.enabled})
	}

	handleChange(uri){
		this.setState({uri: uri})

		var allowed_types = ['artist','track']
		if (allowed_types.indexOf(helpers.uriType(uri)) > -1){
			this.setState({uri_validated: true})
		}
	}

	save(){
		this.props.pusherActions.startRadio(this.state.seeds)
		this.props.uiActions.closeModal()
	}

	stop(){
		this.props.pusherActions.stopRadio()
		this.props.uiActions.closeModal()
	}

	addSeed(){
		if (!this.state.uri_validated || this.state.uri == '') return null

		var seeds = Object.assign([],this.state.seeds)

		if (seeds.indexOf(this.state.uri) <= -1){
			seeds.push(this.state.uri)
		} else {
			this.props.uiActions.createNotification('Seed already exists','bad')
		}

		// commit to state
		this.setState({
			seeds: seeds,
			uri: '',
			uri_validated: false
		})
	}

	removeSeed(uri){
		var seeds = Object.assign([],this.state.seeds)
		var index = seeds.indexOf(uri)
		if (index > -1){
			delete seeds[index]
			this.setState({seeds: seeds})
		}
	}

	renderSeeds(){
/*
		var seeds = this.props.radio.resolved_seeds
		var uri

		var seeds = []
		for (var i = 0; i < this.props.radio.seed_tracks.length; i++){
			var uri = this.props.radio.seed_tracks[i]
			if (this.props.radio.resolved_seeds.hasOwnProperty(uri)){
				seeds.push(this.props.radio.resolved_seeds[uri])
			}
		}
		for (var i = 0; i < this.props.radio.seed_artists.length; i++){
			var uri = this.props.radio.seed_artists[i]
			if (this.props.radio.resolved_seeds.hasOwnProperty(uri)){
				seeds.push(this.props.radio.resolved_seeds[uri])
			}
		}*/

		return (
			<div className="list">
				{
					this.state.seeds.map((seed,index) => {
						return (
							<div className="list-item" key={index+'_'+seed}>
								{seed}
								<FontAwesome name="close" className="pull-right destructive" onClick={() => this.removeSeed(seed)} />
							</div>
						)
					})
				}
			</div>
		)
	}

	renderActions(){
		if (this.state.enabled){
			return (
				<span>
					<button className="primary wide" onClick={e => this.save()}>Save</button>
					<button className="destructive wide" onClick={e => this.stop()}>Stop</button>
				</span>
			)
		}else{
			return (
				<span>				
					<button className="primary wide" onClick={e => this.save()}>Start</button>
				</span>
			)
		}
	}

	render(){
		return (
			<div>
				<h4>Edit radio</h4>
				<form>

					<div className="field">
						<input 
							type="text"
							placeholder="Spotify URI"
							onChange={e => this.handleChange(e.target.value)} 
							value={this.state.uri} />
						<button disabled={!this.state.uri_validated} onClick={e => this.addSeed()}>Add</button>
					</div>

					{this.renderSeeds()}

					<div className="actions centered-text">
						{this.renderActions()}
					</div>
				</form>
			</div>
		)
	}
}