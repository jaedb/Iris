
import React, { PropTypes } from 'react';;

import Icon from '../Icon';
import * as helpers from '../../helpers';

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
		e.preventDefault();

		var valid_seeds = true;
		var seeds = this.mapSeeds();
		for (var i = 0; i < seeds.length; i++){
			if (seeds[i].unresolved !== undefined){
				valid_seeds = false;
				continue;
			}
		}

		if (valid_seeds){
			this.props.pusherActions.startRadio(this.state.seeds);
			this.props.uiActions.closeModal();
		} else {
			this.setState({error_message: "Invalid seed URI(s)"});
		}
	}

	handleUpdate(e){
		e.preventDefault();

		var valid_seeds = true;
		var seeds = this.mapSeeds();
		for (var i = 0; i < seeds.length; i++){
			if (seeds[i].unresolved !== undefined){
				valid_seeds = false;
				continue;
			}
		}

		if (valid_seeds){
			this.props.pusherActions.updateRadio(this.state.seeds);
			this.props.uiActions.closeModal();
		} else {
			this.setState({error_message: "Invalid seed URI(s)"});
		}
	}

	handleStop(e){
		e.preventDefault()
		this.props.pusherActions.stopRadio()
		this.props.uiActions.closeModal()
	}

	addSeed(){
		if (this.state.uri == ''){
			this.setState({error_message: 'Cannot be empty'});
			return;
		}

		var seeds = Object.assign([],this.state.seeds);
		var uris = this.state.uri.split(',');

		for (var i = 0; i < uris.length; i++){
			if (helpers.uriSource(uris[i]) !== 'spotify'){
				this.setState({error_message: 'Non-Spotify URIs not supported'});
				return;
			}
			if (seeds.indexOf(uris[i]) > -1){
				this.setState({error_message: 'URI already added'});
			} else {
				seeds.push(uris[i]);
				this.setState({error_message: null});
			}		

			// Resolve
			switch (helpers.uriType(uris[i])){
				case 'track':
					this.props.spotifyActions.getTrack(uris[i]);
					break;

				case 'artist':
					this.props.spotifyActions.getArtist(uris[i]);
					break;
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
		this.setState({seeds: seeds});
	}

	mapSeeds(){		
		var seeds = []

		if (this.state.seeds){
			for (var i = 0; i < this.state.seeds.length; i++){
				var uri = this.state.seeds[i]
				if (uri){
					if (helpers.uriType(uri) == 'artist'){
						if (this.props.artists && this.props.artists.hasOwnProperty(uri)){
							seeds.push(this.props.artists[uri]);
						} else {
							seeds.push({
								unresolved: true,
								uri: uri
							})
						}
					} else if (helpers.uriType(uri) == 'track'){
						if (this.props.tracks && this.props.tracks.hasOwnProperty(uri)){
							seeds.push(this.props.tracks[uri]);
						} else {
							seeds.push({
								unresolved: true,
								uri: uri
							})
						}
					}
				}
			}
		}

		return seeds;
	}

	renderSeeds(){
		var seeds = this.mapSeeds();

		if (seeds.length > 0){
			return (
				<div>
					<div className="list">
						{
							seeds.map((seed,index) => {
								return (
									<div className="list-item" key={seed.uri}>
										{seed.unresolved ? <span className="grey-text">{seed.uri}</span> : <span>{seed.name}</span> }
										{!seed.unresolved ? <span className="grey-text">&nbsp;({seed.type})</span> : null}
										<button className="discrete remove-uri no-hover"  onClick={e => this.removeSeed(seed.uri)}>
											<Icon name="delete" />Remove
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
				<h1>Radio</h1>
				<h2 className="grey-text">Add and remove seeds to shape the sound of your radio. Radio uses Spotify's recommendations engine to suggest tracks similar to your seeds.</h2>

				<form>
					{this.renderSeeds()}

					<div className="field text">
						<span className="label">URI(s)</span>
						<input 
							type="text"
							onChange={e => this.setState({uri: e.target.value, error_message: null})} 
							value={this.state.uri} />
						<span className="button discrete add-uri no-hover" onClick={e => this.addSeed()}>
							<Icon name="add" />Add
						</span>
						{this.state.error_message ? <span className="description error">{this.state.error_message}</span> : null}
					</div>
				</form>

				<form>
					<div className="actions centered-text">
						{this.state.enabled ? <button className="destructive large" onClick={e => this.handleStop(e)}>Stop</button> : null}
						{this.state.enabled ? <button className="primary large" onClick={e => this.handleUpdate(e)}>Save</button> : <button className="primary large" onClick={e => this.handleStart(e)}>Start</button>}
					</div>
				</form>
			</div>
		)
	}
}