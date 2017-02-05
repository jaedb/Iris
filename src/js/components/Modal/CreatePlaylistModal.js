
import React, { PropTypes } from 'react'

import Icon from '../Icon'
import * as helpers from '../../helpers'

export default class CreatePlaylistModal extends React.Component{

	constructor(props){
		super(props)
		this.state = {
			submit_enabled: false,
			name: '',
			scheme: 'spotify',
			is_public: true
		}
	}

	createPlaylist(e){		
		e.preventDefault();		
		this.props.uiActions.createPlaylist( this.state.scheme, this.state.name, this.state.is_public )
		this.props.uiActions.closeModal()
		return false;
	}

	setPlaylistName(name){
		var submit_enabled = false
		if( name && name != '' ) submit_enabled = true
		this.setState({
			name: name,
			submit_enabled: submit_enabled
		})
	}

	render(){
		return (
			<div>
				<h4>Create playlist</h4>
				<form onSubmit={(e) => this.createPlaylist(e)}>
					<div className="field">
						<input 
							type="text"
							placeholder="Playlist name"
							onChange={ e => this.setPlaylistName( e.target.value )} 
							value={ this.state.name } />
					</div>
					<div className="field radio white">
						<label>
							<input 
								type="radio"
								name="scheme"
								value="spotify"
								checked={ this.state.scheme == 'spotify' }
								onChange={ e => this.setState({ scheme: e.target.value })} />
							<span className="label">Spotify</span>
						</label>
						<label>
							<input 
								type="radio"
								name="scheme"
								value="m3u"
								checked={ this.state.scheme == 'm3u' }
								onChange={ e => this.setState({ scheme: e.target.value })} />
							<span className="label">Local (m3u)</span>
						</label>
					</div>
					<div className="field checkbox white">
						<label>
							<input 
								type="checkbox"
								name="is_public"
								checked={ this.state.is_public }
								onChange={ e => this.setState({ is_public: !this.state.is_public })} />
							<span className="label">Public</span>
						</label>
					</div>
					<div className="actions centered-text">
						<button type="submit" className="primary wide" disabled={!this.state.submit_enabled}>Save</button>
					</div>
				</form>
			</div>
		)
	}
}