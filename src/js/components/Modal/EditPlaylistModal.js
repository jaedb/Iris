
import React from 'react'

import Icon from '../Icon'
import * as helpers from '../../helpers'

export default class EditPlaylistModal extends React.Component{

	constructor(props){
		super(props)
		this.state = {
			submit_enabled: true,
			name: this.props.data.name,
			is_public: this.props.data.is_public
		}
	}

	setPlaylistName(name){
		var submit_enabled = false
		if( name && name != '' ) submit_enabled = true
		this.setState({
			name: name,
			submit_enabled: submit_enabled
		})
	}

	savePlaylist(e){		
		e.preventDefault();
		this.props.uiActions.savePlaylist(this.props.data.uri, this.state.name, this.state.is_public)
		this.props.uiActions.closeModal()
		return false;
	}

	render(){
		return (
			<div>
				<h4>Edit playlist</h4>
				<form onSubmit={(e) => this.savePlaylist(e)}>
					<div className="field">
						<input 
							type="text"
							placeholder="Playlist name"
							onChange={ e => this.setPlaylistName( e.target.value )} 
							value={ this.state.name } />
					</div>
					<div className="field checkbox white">
						<label>
							<input 
								type="checkbox"
								name="playlist_private"
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
