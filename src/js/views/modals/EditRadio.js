
import React from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import Link from '../../components/Link'
import ReactGA from 'react-ga'

import Modal from './Modal';
import Icon from '../../components/Icon';

import * as coreActions from '../../services/core/actions'
import * as uiActions from '../../services/ui/actions'
import * as mopidyActions from '../../services/mopidy/actions'
import * as spotifyActions from '../../services/spotify/actions'
import * as pusherActions from '../../services/pusher/actions'
import * as helpers from '../../helpers';

class EditRadio extends React.Component{

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
		this.props.uiActions.setWindowTitle("Edit radio");

		if (this.props.radio && this.props.radio.enabled){
			this.loadRadio(this.props.radio);
		}
	}

	componentWillReceiveProps(nextProps){
		if (!this.props.radio && nextProps.radio){
			this.loadRadio(nextProps.radio);
		}
	}

	loadRadio(radio){
		var seeds = [...radio.seed_tracks, ...radio.seed_artists, ...radio.seed_genres];
		this.setState({seeds: seeds, enabled: radio.enabled});
		this.props.spotifyActions.resolveRadioSeeds(radio);
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
			window.history.back();
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

	addSeed(e){
		e.preventDefault();

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
									<div className="list__item" key={seed.uri}>
										{seed.unresolved ? <span className="mid_grey-text">{seed.uri}</span> : <span>{seed.name}</span> }
										{!seed.unresolved ? <span className="mid_grey-text">&nbsp;({helpers.uriType(seed.uri)})</span> : null}
										<span className="button discrete remove-uri no-hover" onClick={e => this.removeSeed(seed.uri)}>
											<Icon name="delete" />Remove
										</span>
									</div>
								)
							})
						}
					</div>
				</div>
			)
		} else {
			return (
				<div className="no-results">No seeds</div>
			)
		}
	}

	render(){
		return (
			<Modal className="modal--edit-radio">
				<h1>Radio</h1>
				<h2 className="mid_grey-text">Add and remove seeds to shape the sound of your radio. Radio uses Spotify's recommendations engine to suggest tracks similar to your seeds.</h2>

				<form onSubmit={e => {(this.state.enabled ? this.handleUpdate(e) : this.handleStart(e))}}>

					{this.renderSeeds()}

					<div className="field text">
						<div className="name">URI(s)</div>
						<div className="input">
							<input 
								type="text"
								onChange={e => this.setState({uri: e.target.value, error_message: null})} 
								value={this.state.uri}
							/>
							<span className="button discrete add-uri no-hover" onClick={e => this.addSeed(e)}>
								<Icon name="add" />Add
							</span>
							{this.state.error_message ? <span className="description error">{this.state.error_message}</span> : null}
						</div>
					</div>

					<div className="actions centered-text">
						{this.state.enabled ? <span className="button button--destructive button--large" onClick={e => this.handleStop(e)}>Stop</span> : null}

						{this.state.enabled ? <button className="button button--primary button--large" onClick={e => this.handleUpdate(e)}>Save</button> : <button className="button button--primary button--large" onClick={e => this.handleStart(e)}>Start</button>}
					</div>
				</form>
			</Modal>
		)
	}
}

const mapStateToProps = (state, ownProps) => {
	return {
		mopidy_connected: state.mopidy.connected,
		radio: state.core.radio,
		artists: state.core.artists,
		tracks: state.core.tracks
	}
}

const mapDispatchToProps = (dispatch) => {
	return {
		coreActions: bindActionCreators(coreActions, dispatch),
		uiActions: bindActionCreators(uiActions, dispatch),
		mopidyActions: bindActionCreators(mopidyActions, dispatch),
		pusherActions: bindActionCreators(pusherActions, dispatch),
		spotifyActions: bindActionCreators(spotifyActions, dispatch)
	}
}

export default connect(mapStateToProps, mapDispatchToProps)(EditRadio)