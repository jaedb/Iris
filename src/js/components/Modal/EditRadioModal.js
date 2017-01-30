
import React, { PropTypes } from 'react'
import FontAwesome from 'react-fontawesome'

import Icon from '../Icon'
import * as helpers from '../../helpers'

export default class EditRadioModal extends React.Component{

	constructor(props){
		super(props)
		this.state = {
			seed_tracks: [],
			seed_genres: [],
			seed_artists: [],
			uri: ''
		}
	}

	componentDidMount(){
		this.setState(this.props.radio)
	}

	save(){
		this.props.uiActions.closeModal()
	}

	addSeed(seed){
		this.setState({uri: ''})
	}

	removeSeed(type,seed){
		console.log('remove', type, seed)
	}

	renderSeeds(){
		if (!this.props.radio || !this.props.radio.enabled) return null
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
					this.state.seed_tracks.map(seed => {
						return (
							<div className="list-item" key={seed}>
								{seed}
								<FontAwesome name="close" className="pull-right destructive" onClick={() => this.removeSeed('track',seed)} />
							</div>
						)
					})
				}
				{
					this.state.seed_genres.map(seed => {
						return (
							<div className="list-item" key={seed}>
								{seed}
								<FontAwesome name="close" className="pull-right destructive" onClick={() => this.removeSeed('genre',seed)} />
							</div>
						)
					})
				}
				{
					this.state.seed_artists.map(seed => {
						return (
							<div className="list-item" key={seed}>
								{seed}
								<FontAwesome name="close" className="pull-right destructive" onClick={() => this.removeSeed('artist',seed)} />
							</div>
						)
					})
				}
			</div>
		)
	}

	render(){
		return (
			<div>
				<h4>Edit radio</h4>
				<form>
					{this.renderSeeds()}
					<div className="field">
						<input 
							type="text"
							placeholder="URI"
							onChange={e => this.setState({uri: e.target.value})} 
							value={this.state.uri} />
					</div>
					<button className="secondary" onClick={e => this.addSeed(this.state.uri)}>Add</button>
					<button type="submit" className="primary" onClick={e => this.save()}>Save and close</button>
				</form>
			</div>
		)
	}
}